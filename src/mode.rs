use crate::{
    app::AppState, installer::InstallMode, pob::PoBMode, renderer::primitives::ClippedPrimitive,
};
use winit::{event::MouseButton, keyboard::KeyCode};

pub enum AppEvent {
    KeyDown {
        code: KeyCode,
    },
    KeyUp {
        code: KeyCode,
    },
    MouseDown {
        button: MouseButton,
        is_double_click: bool,
    },
    MouseUp {
        button: MouseButton,
    },
    MouseWheel {
        delta: f32,
    },
    CharacterInput {
        ch: char,
    },
    Exit,
}

/// Represents the transition to another mode
pub enum ModeTransition {
    PoB,
}

pub struct ModeFrameOutput {
    pub primitives: Box<dyn Iterator<Item = ClippedPrimitive>>,
    pub can_elide: bool,
    /// Indicates that this should be redrawn again next frame even if user is not interacting with
    /// window
    pub should_continue: bool,
}

pub enum AppMode {
    Install(InstallMode),
    PoB(PoBMode),
}

impl AppMode {
    pub fn frame(&mut self, state: &mut AppState) -> anyhow::Result<ModeFrameOutput> {
        match self {
            AppMode::Install(mode) => mode.frame(state),
            AppMode::PoB(mode) => mode.frame(state),
        }
    }

    pub fn update(&mut self, state: &mut AppState) -> anyhow::Result<Option<ModeTransition>> {
        match self {
            AppMode::Install(mode) => mode.update(state),
            AppMode::PoB(mode) => mode.update(state),
        }
    }

    pub fn handle_event(&mut self, state: &mut AppState, event: AppEvent) -> anyhow::Result<()> {
        match self {
            AppMode::Install(mode) => mode.handle_event(state, event),
            AppMode::PoB(mode) => mode.handle_event(state, event),
        }
    }
}
