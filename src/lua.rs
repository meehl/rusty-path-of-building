use crate::{
    Game,
    api::{self, get_callback},
    app::AppState,
    args::Args,
    fonts::Fonts,
    input::InputState,
    layers::Layers,
    pob::PoBState,
    renderer::textures::WrappedTextureManager,
    subscript::{NativeMultiValue, SubscriptManager, SubscriptResult, register_subscript_globals},
    util::change_working_directory,
    window::WindowState,
};
use arboard::Clipboard;
use clap::Parser;
use mlua::{Lua, Result as LuaResult, Table};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
};

macro_rules! ctx_accessor {
    ($field:ident: & $ty:ty) => {
        pub fn $field(&self) -> &$ty {
            let ptr = self.$field.get();
            assert!(!ptr.is_null());
            unsafe { &*ptr }
        }
    };

    ($field:ident: &mut $ty:ty) => {
        #[allow(clippy::mut_from_ref)]
        pub fn $field(&self) -> &mut $ty {
            let ptr = self.$field.get();
            assert!(!ptr.is_null());
            unsafe { &mut *ptr }
        }
    };
}

/// A collection of pointers needed by the API funtions.
///
/// Before executing any lua code, we need to "plug" the references into
/// the Context and "unplug" them afterwards.
pub struct Context {
    window: Cell<*const WindowState>,
    input: Cell<*const InputState>,
    fonts: Cell<*mut Fonts>,
    texture_manager: Cell<*mut WrappedTextureManager>,
    clipboard: Cell<*mut Clipboard>,
    current_working_dir: Cell<*mut PathBuf>,
    layers: Cell<*mut Layers>,
    needs_restart: Cell<*mut bool>,
    is_dpi_aware: Cell<*mut bool>,
}

impl Context {
    pub fn new() -> &'static Self {
        Box::leak(Box::new(Self {
            window: Cell::new(std::ptr::null()),
            input: Cell::new(std::ptr::null()),
            fonts: Cell::new(std::ptr::null_mut()),
            texture_manager: Cell::new(std::ptr::null_mut()),
            clipboard: Cell::new(std::ptr::null_mut()),
            current_working_dir: Cell::new(std::ptr::null_mut()),
            layers: Cell::new(std::ptr::null_mut()),
            needs_restart: Cell::new(std::ptr::null_mut()),
            is_dpi_aware: Cell::new(std::ptr::null_mut()),
        }))
    }

    pub fn set(&self, ctx: &mut PoBContext) {
        self.window.set(&ctx.app.window);
        self.input.set(&ctx.app.input);
        self.fonts.set(&mut ctx.app.fonts);
        self.texture_manager.set(&mut ctx.app.texture_manager);
        self.clipboard.set(&mut ctx.app.clipboard);
        self.current_working_dir
            .set(&mut ctx.pob.current_working_dir);
        self.layers.set(&mut ctx.pob.layers);
        self.needs_restart.set(&mut ctx.pob.needs_restart);
        self.is_dpi_aware.set(&mut ctx.pob.is_dpi_aware);
    }

    pub fn clear(&self) {
        self.window.set(std::ptr::null());
        self.input.set(std::ptr::null());
        self.fonts.set(std::ptr::null_mut());
        self.texture_manager.set(std::ptr::null_mut());
        self.clipboard.set(std::ptr::null_mut());
        self.current_working_dir.set(std::ptr::null_mut());
        self.layers.set(std::ptr::null_mut());
        self.needs_restart.set(std::ptr::null_mut());
        self.is_dpi_aware.set(std::ptr::null_mut());
    }

    ctx_accessor!(window: &WindowState);
    ctx_accessor!(input: &InputState);
    ctx_accessor!(fonts: &mut Fonts);
    ctx_accessor!(texture_manager: &mut WrappedTextureManager);
    ctx_accessor!(clipboard: &mut Clipboard);
    ctx_accessor!(current_working_dir: &mut PathBuf);
    ctx_accessor!(layers: &mut Layers);
    ctx_accessor!(needs_restart: &mut bool);
    ctx_accessor!(is_dpi_aware: &mut bool);
}

pub enum PoBEvent {
    Init,
    Exit,
    Frame,
    KeyDown(&'static str, bool),
    KeyUp(&'static str),
    Char(char),
    SubFinished {
        id: u64,
        return_values: NativeMultiValue,
    },
    SubError {
        id: u64,
        error: String,
    },
}

impl std::fmt::Display for PoBEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PoBEvent::Init => write!(f, "Init"),
            PoBEvent::Exit => write!(f, "Exit"),
            PoBEvent::Frame => write!(f, "Frame"),
            PoBEvent::KeyDown(_, _) => write!(f, "KeyDown"),
            PoBEvent::KeyUp(_) => write!(f, "KeyUp"),
            PoBEvent::Char(_) => write!(f, "Char"),
            PoBEvent::SubFinished { .. } => write!(f, "SubFinished"),
            PoBEvent::SubError { .. } => write!(f, "SubError"),
        }
    }
}

pub struct PoBContext<'a> {
    pub app: &'a mut AppState,
    pub pob: &'a mut PoBState,
}

impl<'a> PoBContext<'a> {
    pub fn new(app_state: &'a mut AppState, pob_state: &'a mut PoBState) -> Self {
        Self {
            app: app_state,
            pob: pob_state,
        }
    }
}

/// Lua instance that runs the PoB application code and manages subscripts.
pub struct LuaInstance {
    lua: Lua,
    subscript_manager: Rc<RefCell<SubscriptManager>>,
}

impl LuaInstance {
    pub fn new() -> anyhow::Result<Self> {
        let subscript_manager = Rc::new(RefCell::new(SubscriptManager::new()));

        let lua = Self::create_lua_state()?;
        register_subscript_globals(&lua, &subscript_manager)?;

        Ok(Self {
            lua,
            subscript_manager,
        })
    }

    fn create_lua_state() -> LuaResult<Lua> {
        // SAFETY: use `unsafe_new` to allow loading of C modules
        let lua = unsafe { Lua::unsafe_new() };

        // expose build arg to lua
        let args = Args::parse();
        let args_table = lua.create_sequence_from(std::iter::once(args.build_path))?;
        lua.globals().set("arg", args_table)?;

        Self::register_package_paths(&lua)?;

        // register context
        let ctx = Context::new();
        lua.set_app_data(ctx);

        // register callbacks
        api::register_globals(&lua)?;

        Ok(lua)
    }

    /// Loads and executes PoB's Launch.lua script
    pub fn launch(&self, pob_ctx: &mut PoBContext) -> LuaResult<()> {
        let ctx = self.lua.app_data_ref::<&'static Context>().unwrap();
        ctx.set(pob_ctx);

        let script_dir = Game::script_dir();
        change_working_directory(script_dir.as_path())?;
        self.load(script_dir.join("Launch.lua")).exec()?;

        ctx.clear();
        Ok(())
    }

    pub fn restart(&mut self, ctx: &mut PoBContext) -> LuaResult<()> {
        self.lua = Self::create_lua_state()?;
        register_subscript_globals(&self.lua, &self.subscript_manager)?;
        self.launch(ctx)?;
        Ok(())
    }

    /// Run functions for subscripts and handle their completion/failure.
    pub fn handle_subscripts(&self, ctx: &mut PoBContext) {
        profiling::scope!("handle_subscripts");

        let subscript_events = self.subscript_manager.borrow_mut().process(self);

        // Handle finished/errored subscripts.
        for event in subscript_events {
            match event {
                SubscriptResult::SubscriptFinished { id, return_values } => {
                    self.handle_event(PoBEvent::SubFinished { id, return_values }, ctx)
                        .unwrap();
                }
                SubscriptResult::SubscriptError { id, error } => {
                    self.handle_event(PoBEvent::SubError { id, error }, ctx)
                        .unwrap();
                }
            }
        }
    }

    pub fn handle_event(&self, event: PoBEvent, pob_ctx: &mut PoBContext) -> LuaResult<()> {
        profiling::scope!("handle_event", format!("{}", event));

        // "Plug" references into context
        let ctx = self.lua.app_data_ref::<&'static Context>().unwrap();
        ctx.set(pob_ctx);

        // Call event handler in PoB application code
        let handler_result = match event {
            PoBEvent::Init => get_callback(&self.lua, "OnInit")?.call::<()>(()),
            PoBEvent::Exit => get_callback(&self.lua, "OnExit")?.call::<()>(()),
            PoBEvent::Frame => get_callback(&self.lua, "OnFrame")?.call::<()>(()),
            PoBEvent::KeyDown(key, double_click) => {
                get_callback(&self.lua, "OnKeyDown")?.call::<()>((key, double_click))
            }
            PoBEvent::KeyUp(key) => get_callback(&self.lua, "OnKeyUp")?.call::<()>(key),
            PoBEvent::Char(ch) => get_callback(&self.lua, "OnChar")?.call::<()>(ch),
            PoBEvent::SubFinished { id, return_values } => {
                get_callback(&self.lua, "OnSubFinished")?.call::<()>((id, return_values))
            }
            PoBEvent::SubError { id, error } => {
                get_callback(&self.lua, "OnSubError")?.call::<()>((id, error))
            }
        };

        // "Unplug" references from context
        ctx.clear();

        handler_result
    }

    /// Adds "${script_dir}/lua" to package path
    pub fn register_package_paths(lua: &Lua) -> LuaResult<()> {
        let script_dir = Game::script_dir();
        let package: Table = lua.globals().get("package")?;
        let mut package_path: String = package.get("path")?;
        package_path.push(';');
        package_path.push_str(script_dir.join("lua/?.lua").to_str().unwrap());
        package_path.push(';');
        package_path.push_str(script_dir.join("lua/?/init.lua").to_str().unwrap());
        package.set("path", package_path)?;

        Ok(())
    }
}

impl std::ops::Deref for LuaInstance {
    type Target = Lua;
    fn deref(&self) -> &Self::Target {
        &self.lua
    }
}
