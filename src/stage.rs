use std::sync::Arc;

use winit::window::Window;

use crate::{
    dpi::{PhysicalPoint, PhysicalSize},
    draw_commands::DrawCommand,
    installer::Installer,
    pob::PathOfBuilding,
};

pub type Key = winit::keyboard::Key;
pub type MouseButton = winit::event::MouseButton;
pub type ModifiersState = winit::keyboard::ModifiersState;

pub enum StageEvent {
    KeyDown {
        key: Key,
    },
    KeyUp {
        key: Key,
    },
    CharacterInput {
        ch: char,
    },
    ModifiersChanged {
        state: ModifiersState,
    },
    MouseDown {
        button: MouseButton,
        //is_double_click: bool,
    },
    MouseUp {
        button: MouseButton,
    },
    MouseWheel {
        delta: f32,
    },
    MouseMoved {
        pos: PhysicalPoint<f32>,
    },
    Exit,
    Resized(PhysicalSize<u32>),
    ScaleFactorChanged(f64),
    FocusChanged(bool),
    HoverChanged(bool),
}

pub enum StageTransition {
    ToMain,
    ToShutdown,
}

pub struct StageFrameOutput {
    pub draw_commands: Vec<DrawCommand>,
    pub can_elide: bool,
    pub request_redraw: bool,
    pub scale_factor: f32,
}

pub enum ActiveStage {
    Startup(Installer),
    Main(PathOfBuilding),
}

impl ActiveStage {
    pub fn update(&mut self) -> anyhow::Result<Option<StageTransition>> {
        match self {
            ActiveStage::Startup(s) => s.update(),
            ActiveStage::Main(s) => s.update(),
        }
    }

    pub fn frame(&mut self) -> anyhow::Result<StageFrameOutput> {
        match self {
            ActiveStage::Startup(s) => s.frame(),
            ActiveStage::Main(s) => s.frame(),
        }
    }

    pub fn handle_event(&mut self, event: StageEvent) -> anyhow::Result<Option<StageTransition>> {
        match self {
            ActiveStage::Startup(s) => s.handle_event(event),
            ActiveStage::Main(s) => s.handle_event(event),
        }
    }

    pub fn set_window(&mut self, window: Arc<Window>) {
        match self {
            ActiveStage::Startup(s) => s.set_window(window),
            ActiveStage::Main(s) => s.set_window(window),
        }
    }

    pub fn can_exit(&mut self) -> bool {
        match self {
            Self::Startup(_) => true,
            Self::Main(s) => s.can_exit(),
        }
    }

    pub fn clear_pressed(&mut self) {
        match self {
            Self::Startup(_) => {}
            Self::Main(s) => s.clear_pressed(),
        }
    }
}
