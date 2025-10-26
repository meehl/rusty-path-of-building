use crate::{
    app::AppState,
    dpi::{LogicalRect, LogicalSize},
    input::{keycode_as_str, mousebutton_as_str},
    layers::Layers,
    lua::{LuaInstance, PoBContext, PoBEvent},
    mode::{AppEvent, ModeFrameOutput, ModeTransition},
};
use std::path::PathBuf;

pub struct PoBState {
    pub layers: Layers,
    pub current_working_dir: PathBuf,
    pub needs_restart: bool,
}

/// Execution mode in which PoB's application code is run.
///
/// It forwards app events to PoB's application event handlers and outputs
/// the draw primitives created by PoB each frame.
pub struct PoBMode {
    lua_instance: LuaInstance,
    state: PoBState,
    previous_layers_hash: u64,
}

impl PoBMode {
    pub fn new(app_state: &mut AppState) -> anyhow::Result<Self> {
        let mut state = PoBState {
            layers: Layers::default(),
            current_working_dir: PathBuf::default(),
            needs_restart: false,
        };

        let lua_instance = LuaInstance::new()?;

        let mut pob_ctx = PoBContext::new(app_state, &mut state);
        lua_instance.launch(&mut pob_ctx)?;
        lua_instance.handle_event(PoBEvent::Init, &mut pob_ctx)?;

        Ok(Self {
            lua_instance,
            state,
            previous_layers_hash: Default::default(),
        })
    }

    pub fn frame(&mut self, app_state: &mut AppState) -> anyhow::Result<ModeFrameOutput> {
        profiling::scope!("frame");

        // reset layers and viewport
        self.state.layers.reset();
        self.reset_viewport(app_state.window.logical_size());

        let mut ctx = PoBContext::new(app_state, &mut self.state);

        // handle subscripts
        self.lua_instance.handle_subscripts(&mut ctx);

        // run PoB's draw code.
        // this will "fill up" up the layers with draw primitives
        self.lua_instance.handle_event(PoBEvent::Frame, &mut ctx)?;

        // check if draw prmitives are identical to primitives from last frame
        let layers_hash = self.state.layers.get_hash();
        let identical = layers_hash == self.previous_layers_hash;
        self.previous_layers_hash = layers_hash;

        Ok(ModeFrameOutput {
            primitives: self.state.layers.consume_layers(),
            can_elide: identical,
        })
    }

    pub fn update(&mut self, app_state: &mut AppState) -> anyhow::Result<Option<ModeTransition>> {
        if self.state.needs_restart {
            let mut ctx = PoBContext::new(app_state, &mut self.state);
            self.lua_instance.restart(&mut ctx)?;
            self.lua_instance.handle_event(PoBEvent::Init, &mut ctx)?;
            self.state.needs_restart = false;
        }
        Ok(None)
    }

    pub fn handle_event(
        &mut self,
        app_state: &mut AppState,
        event: AppEvent,
    ) -> anyhow::Result<()> {
        let mut ctx = PoBContext::new(app_state, &mut self.state);

        match event {
            AppEvent::KeyDown { code } => {
                if let Some(key_string) = keycode_as_str(code) {
                    let pob_event = PoBEvent::KeyDown(key_string, false);
                    self.lua_instance.handle_event(pob_event, &mut ctx)?;
                }
            }
            AppEvent::KeyUp { code } => {
                if let Some(key_string) = keycode_as_str(code) {
                    let pob_event = PoBEvent::KeyUp(key_string);
                    self.lua_instance.handle_event(pob_event, &mut ctx)?;
                }
            }
            AppEvent::MouseDown {
                button,
                is_double_click,
            } => {
                if let Some(button_string) = mousebutton_as_str(button) {
                    let pob_event = PoBEvent::KeyDown(button_string, is_double_click);
                    self.lua_instance.handle_event(pob_event, &mut ctx)?;
                }
            }
            AppEvent::MouseUp { button } => {
                if let Some(button_string) = mousebutton_as_str(button) {
                    let pob_event = PoBEvent::KeyUp(button_string);
                    self.lua_instance.handle_event(pob_event, &mut ctx)?;
                }
            }
            AppEvent::MouseWheel { delta } => {
                if delta > 0.0 {
                    self.lua_instance
                        .handle_event(PoBEvent::KeyUp("WHEELUP"), &mut ctx)?;
                } else if delta < 0.0 {
                    self.lua_instance
                        .handle_event(PoBEvent::KeyUp("WHEELDOWN"), &mut ctx)?;
                }
            }
            AppEvent::CharacterInput { ch } => {
                self.lua_instance
                    .handle_event(PoBEvent::Char(ch), &mut ctx)?;
            }
            AppEvent::Exit => self.lua_instance.handle_event(PoBEvent::Exit, &mut ctx)?,
        }
        Ok(())
    }

    fn reset_viewport(&mut self, size: LogicalSize<u32>) {
        self.state
            .layers
            .set_viewport(LogicalRect::from_size(size).cast());
    }
}
