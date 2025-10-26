use std::sync::Arc;

use winit::window::Window;

use crate::dpi::{ConvertToLogical, LogicalSize, PhysicalSize};

pub struct WindowState {
    pub window: Option<Arc<Window>>,
    pub size: PhysicalSize<u32>,
    pub scale_factor: f32,
    pending_window_title: std::cell::Cell<Option<String>>,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            window: None,
            size: Default::default(),
            scale_factor: 1.0,
            pending_window_title: std::cell::Cell::new(None),
        }
    }
}

impl WindowState {
    pub fn set_window(&mut self, window: Arc<Window>) {
        if let Some(title) = self.pending_window_title.take() {
            window.set_title(&title);
        }

        self.window = Some(window);
    }

    pub fn set_window_title(&self, title: &str) {
        if let Some(ref window) = self.window {
            window.set_title(title);
        } else {
            self.pending_window_title.set(Some(title.to_string()));
        }
    }

    pub fn logical_size(&self) -> LogicalSize<u32> {
        self.size.to_logical(self.scale_factor)
    }
}
