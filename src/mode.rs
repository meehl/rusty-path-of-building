use crate::{
    app::AppState, installer::InstallMode, pob::PoBMode, renderer::primitives::ClippedPrimitive,
};
use winit::{event::MouseButton, keyboard::Key};

pub enum AppEvent {
    KeyDown {
        key: Key,
    },
    KeyUp {
        key: Key,
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
            Self::Install(mode) => mode.frame(state),
            Self::PoB(mode) => mode.frame(state),
        }
    }

    pub fn update(&mut self, state: &mut AppState) -> anyhow::Result<Option<ModeTransition>> {
        match self {
            Self::Install(mode) => mode.update(state),
            Self::PoB(mode) => mode.update(state),
        }
    }

    pub fn handle_event(&mut self, state: &mut AppState, event: AppEvent) -> anyhow::Result<()> {
        match self {
            Self::Install(mode) => mode.handle_event(state, event),
            Self::PoB(mode) => mode.handle_event(state, event),
        }
    }

    pub fn can_exit(&mut self, state: &mut AppState) -> bool {
        match self {
            Self::Install(_) => true,
            Self::PoB(mode) => mode.can_exit(state),
        }
    }
}
