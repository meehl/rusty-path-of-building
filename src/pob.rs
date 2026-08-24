use clap::Parser;
use mlua::{
    AppDataRef, AppDataRefMut, Function, Lua, Result as LuaResult, Table, thread::ThreadStatus,
};
use winit::window::Window;

use crate::{
    args::Args,
    dpi::PhysicalPoint,
    draw_commands::DrawCommandRecorder,
    fonts::Fonts,
    input::{InputState, key_as_str, mousebutton_as_str},
    pob::{
        api::call_callback,
        subscript::{NativeMultiValue, SubscriptManager, SubscriptResult},
    },
    renderer::textures::WrappedTextureManager,
    stage::{StageEvent, StageFrameOutput, StageTransition},
    util::change_working_directory,
    window::WindowState,
};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

mod api;
mod subscript;

/// Holds all state shared between Rust and the API functions exposed to Lua.
///
/// Accessed  via `lua.app_data_ref::<Context>()`/`app_data_mut::<Context>()`.
pub struct Context {
    pub input_state: InputState,
    pub window_state: WindowState,
    /// Collects draw commands issued by API functions
    pub recorder: DrawCommandRecorder,
    pub fonts: Rc<RefCell<Fonts>>,
    pub texture_manager: Rc<RefCell<WrappedTextureManager>>,
    pub subscript_manager: RefCell<SubscriptManager>,
    /// Directory containing the PoB Lua scripts
    pub script_dir: PathBuf,
    pub current_working_dir: PathBuf,
    /// Set by Lua to request a full Lua restart
    pub needs_restart: bool,
    /// Set by Lua to request application shutdown
    pub should_exit: bool,
    pub is_dpi_aware: bool,
}

impl Context {
    pub fn new(
        script_dir: &Path,
        fonts: Rc<RefCell<Fonts>>,
        texture_manager: Rc<RefCell<WrappedTextureManager>>,
    ) -> Self {
        Self {
            input_state: InputState::default(),
            window_state: WindowState::default(),
            recorder: DrawCommandRecorder::default(),
            fonts,
            texture_manager,
            subscript_manager: RefCell::new(SubscriptManager::default()),
            script_dir: script_dir.to_owned(),
            current_working_dir: PathBuf::default(),
            needs_restart: false,
            should_exit: false,
            is_dpi_aware: false,
        }
    }

    /// Resizes the recorder's viewport to match the current window size.
    /// Call at the start of every frame before any draw calls are recorded.
    pub fn set_recorder_viewport_to_window_size(&mut self) {
        self.recorder
            .set_viewport_from_size(self.window_state.logical_size());
    }

    fn set_mouse_pos(&mut self, pos: PhysicalPoint<f32>) {
        self.input_state
            .set_mouse_pos(pos / self.window_state.scale_factor());
    }
}

/// Represents the interface to Path of Building's Lua code.
///
/// Exposes API, forwards events to Path of Building's event handlers, and manages subscripts.
pub struct PathOfBuilding {
    lua: Lua,
    /// Hash of the previous frame's draw commands. Used to skip rerendering identical frames.
    previous_layers_hash: u64,
}

impl PathOfBuilding {
    pub fn new(
        script_dir: &Path,
        fonts: Rc<RefCell<Fonts>>,
        texture_manager: Rc<RefCell<WrappedTextureManager>>,
    ) -> anyhow::Result<Self> {
        let ctx = Context::new(script_dir, fonts, texture_manager);
        let lua = Self::create_lua(ctx)?;

        Self::launch(&lua, script_dir)?;

        Ok(Self {
            lua,
            previous_layers_hash: Default::default(),
        })
    }

    /// Builds a `Lua` state with package paths, context, and all API functions registered.
    fn create_lua(ctx: Context) -> LuaResult<Lua> {
        // `unsafe_new` needed to allow loading of C modules
        let lua = unsafe { Lua::unsafe_new() };

        // expose import url to lua
        let args = Args::parse();
        let args_table = lua.create_sequence_from(std::iter::once(args.import_url))?;
        lua.globals().set("arg", args_table)?;

        Self::register_package_paths(&lua, &ctx.script_dir)?;

        // make context accessible to API functions
        lua.set_app_data(ctx);

        // register API functions
        api::register_globals(&lua)?;

        Ok(lua)
    }

    /// Adds `${script_dir}/lua` to package path
    pub fn register_package_paths(lua: &Lua, script_dir: &Path) -> LuaResult<()> {
        let package: Table = lua.globals().get("package")?;
        let mut package_path: String = package.get("path")?;
        package_path.push(';');
        package_path.push_str(script_dir.join("lua/?.lua").to_str().unwrap());
        package_path.push(';');
        package_path.push_str(script_dir.join("lua/?/init.lua").to_str().unwrap());
        package.set("path", package_path)?;
        Ok(())
    }

    /// Loads and runs `Launch.lua`, then calls PoB's `OnInit` handler.
    fn launch(lua: &Lua, script_dir: &Path) -> LuaResult<()> {
        change_working_directory(script_dir)?;
        lua.load(script_dir.join("Launch.lua")).exec()?;
        call_callback::<(), ()>(lua, "OnInit", ())?;
        Ok(())
    }

    /// Rebuilds the Lua instance from scratch. Preserves the current `Context`.
    fn restart(&mut self) -> LuaResult<()> {
        // move context out of current lua instance...
        let mut ctx = self.lua.remove_app_data::<Context>().unwrap();
        let script_dir = ctx.script_dir.clone();
        ctx.needs_restart = false;

        // and move it into new instance
        self.lua = Self::create_lua(ctx)?;
        Self::launch(&self.lua, &script_dir)?;

        Ok(())
    }

    /// Handles events by updating context and forwarding them to PoB's event handlers.
    pub fn handle_event(&self, event: StageEvent) -> anyhow::Result<Option<StageTransition>> {
        match event {
            StageEvent::KeyDown { key } => {
                self.ctx_mut().input_state.set_key_pressed(&key, true);

                if let Some(key) = key_as_str(key) {
                    call_callback::<(&str, bool), ()>(
                        &self.lua,
                        "OnKeyDown",
                        (key.as_str(), false),
                    )?;
                }
            }
            StageEvent::KeyUp { key } => {
                self.ctx_mut().input_state.set_key_pressed(&key, false);

                if let Some(key) = key_as_str(key) {
                    call_callback::<&str, ()>(&self.lua, "OnKeyUp", key.as_str())?;
                }
            }
            StageEvent::ModifiersChanged { state } => {
                self.ctx_mut().input_state.key_modifiers = state;
            }
            StageEvent::CharacterInput { ch } => {
                let ch = if ch.is_ascii() { ch } else { '?' };
                call_callback::<char, ()>(&self.lua, "OnChar", ch)?;
            }
            StageEvent::MouseDown { button } => {
                let is_double_click = self.ctx_mut().input_state.set_mouse_pressed(button, true);

                if let Some(button) = mousebutton_as_str(button) {
                    call_callback::<(&str, bool), ()>(
                        &self.lua,
                        // PoB treats mouse buttons as keys
                        "OnKeyDown",
                        (button.as_str(), is_double_click),
                    )?;
                }
            }
            StageEvent::MouseUp { button } => {
                self.ctx_mut().input_state.set_mouse_pressed(button, false);

                if let Some(button) = mousebutton_as_str(button) {
                    call_callback::<&str, ()>(
                        &self.lua,
                        // PoB treats mouse buttons as keys
                        "OnKeyUp",
                        button.as_str(),
                    )?;
                }
            }
            StageEvent::MouseWheel { delta } => {
                if delta > 0.0 {
                    call_callback::<(&str, bool), ()>(&self.lua, "OnKeyDown", ("WHEELUP", false))?;
                    call_callback::<&str, ()>(&self.lua, "OnKeyUp", "WHEELUP")?;
                } else if delta < 0.0 {
                    call_callback::<(&str, bool), ()>(
                        &self.lua,
                        "OnKeyDown",
                        ("WHEELDOWN", false),
                    )?;
                    call_callback::<&str, ()>(&self.lua, "OnKeyUp", "WHEELDOWN")?;
                }
            }
            StageEvent::Exit => call_callback(&self.lua, "OnExit", ())?,
            StageEvent::MouseMoved { pos } => {
                self.ctx_mut().set_mouse_pos(pos);
            }
            StageEvent::Resized(size) => {
                self.ctx_mut().window_state.size = size;
            }
            StageEvent::ScaleFactorChanged(scale_factor) => {
                #[allow(clippy::cast_possible_truncation)]
                self.ctx_mut()
                    .window_state
                    .set_scale_factor(scale_factor as f32);
            }
            StageEvent::FocusChanged(focused) => {
                self.ctx_mut().window_state.is_focused = focused;
            }
            StageEvent::HoverChanged(hovered) => {
                self.ctx_mut().window_state.is_hovered = hovered;
            }
        }
        Ok(None)
    }

    /// Applies any pending restart/exit requests set by Lua via `Context`.
    /// Should be called once before `frame()`.
    pub fn update(&mut self) -> anyhow::Result<Option<StageTransition>> {
        let (needs_restart, should_exit) = {
            let ctx = self.ctx();
            (ctx.needs_restart, ctx.should_exit)
        };

        if needs_restart {
            self.restart()?;
        }

        if should_exit {
            return Ok(Some(StageTransition::ToShutdown));
        }

        Ok(None)
    }

    /// Runs one PoB frame.
    ///
    /// Resets the draw command recorder, advances subscripts, calls `OnFrame` (which issues draw
    /// calls via the API), then collects the resulting draw commands.
    ///
    /// Returns `can_elide: true` if the output is identical to the previous frame, letting the
    /// caller skip rerendering.
    pub fn frame(&mut self) -> anyhow::Result<StageFrameOutput> {
        profiling::scope!("frame");

        {
            let mut ctx = self.ctx_mut();
            ctx.recorder.reset();
            ctx.set_recorder_viewport_to_window_size();
        }

        self.handle_subscripts()?;

        {
            profiling::scope!("lua_OnFrame");
            call_callback::<(), ()>(&self.lua, "OnFrame", ())?;
        }

        let ctx = self.lua.app_data_ref::<Context>().unwrap();

        let (layers_hash, layers) = ctx.recorder.finish();
        let identical = layers_hash == self.previous_layers_hash;
        self.previous_layers_hash = layers_hash;

        // determine if another redraw needs to be requested after this frame.
        // this mirrors SimpleGraphic's behavior.
        let is_focused = ctx.window_state.is_focused;
        let is_hovered = ctx.window_state.is_hovered;
        let has_active_subscript = ctx.subscript_manager.borrow().has_running_subscripts();
        let has_active_coroutine = self.has_active_coroutine();
        let request_redraw =
            is_focused || is_hovered || has_active_subscript || has_active_coroutine;

        Ok(StageFrameOutput {
            draw_commands: layers.values().flatten().copied().collect(),
            can_elide: identical,
            request_redraw,
            scale_factor: ctx.window_state.scale_factor(),
        })
    }

    /// Advances all running subscripts and forwards their results to Lua handlers.
    /// Call once per frame.
    fn handle_subscripts(&self) -> LuaResult<()> {
        profiling::scope!("handle_subscripts");

        let subscript_events = self.ctx().subscript_manager.borrow_mut().process(&self.lua);

        // Handle finished/errored subscripts.
        for event in subscript_events {
            match event {
                SubscriptResult::SubscriptFinished { id, return_values } => {
                    call_callback::<(u64, NativeMultiValue), ()>(
                        &self.lua,
                        "OnSubFinished",
                        (id, return_values),
                    )?;
                }
                SubscriptResult::SubscriptError { id, error } => {
                    call_callback::<(u64, String), ()>(&self.lua, "OnSubError", (id, error))?;
                }
            }
        }
        Ok(())
    }

    fn has_active_coroutine(&self) -> bool {
        self.get_coroutines().is_ok_and(|coroutines| {
            coroutines.pairs::<mlua::Thread, bool>().any(|pair| {
                pair.is_ok_and(|(thread, _)| {
                    matches!(
                        thread.status(),
                        ThreadStatus::Resumable | ThreadStatus::Running
                    )
                })
            })
        })
    }

    fn get_coroutines(&self) -> LuaResult<Table> {
        let coroutine_module: Table = self.lua.globals().get("coroutine")?;
        let list_func: Function = coroutine_module.get("_list")?;
        list_func.call::<Table>(())
    }

    pub fn clear_pressed(&self) {
        self.ctx_mut().input_state.clear_pressed();
    }

    /// Calls PoB's `CanExit` handler. Used for unsaved changes prompt.
    pub fn can_exit(&self) -> bool {
        call_callback::<(), bool>(&self.lua, "CanExit", ()).unwrap_or(false)
    }

    /// Attaches the render-target window to the context
    pub fn set_window(&self, window: Arc<Window>) {
        self.ctx_mut().window_state.set_window(window);
    }

    /// Helper for accessing `Context`
    #[inline]
    fn ctx(&self) -> AppDataRef<'_, Context> {
        self.lua.app_data_ref().unwrap()
    }

    /// Helper for getting mutable access to `Context`
    #[inline]
    fn ctx_mut(&self) -> AppDataRefMut<'_, Context> {
        self.lua.app_data_mut().unwrap()
    }
}
