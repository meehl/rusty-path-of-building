use crate::{
    clipboard::Clipboard,
    dpi::{ConvertToLogical, LogicalSize, PhysicalSize},
};
use raw_window_handle::HasDisplayHandle;
use std::sync::Arc;
use winit::window::Window;

pub struct WindowState {
    // NOTE: clipboard needs to be destroyed before window
    clipboard: Option<Clipboard>,
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
            clipboard: None,
        }
    }
}

impl WindowState {
    pub fn set_window(&mut self, window: Arc<Window>) {
        if let Some(title) = self.pending_window_title.take() {
            window.set_title(&title);
        }

        let winit::dpi::PhysicalSize { width, height } = window.inner_size();
        self.size = PhysicalSize::new(width, height);
        self.scale_factor = window.scale_factor() as f32;

        let raw_display_handle = window.display_handle().ok().map(|h| h.as_raw());
        self.clipboard = Some(Clipboard::new(raw_display_handle));
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

    pub fn focus(&self) {
        if let Some(ref window) = self.window {
            window.focus_window();
        }
    }

    pub fn set_clipboard_text(&mut self, text: String) {
        if let Some(clipboard) = &mut self.clipboard {
            clipboard.set_text(text);
        }
    }

    pub fn get_clipboard_text(&mut self) -> Option<String> {
        if let Some(clipboard) = &mut self.clipboard {
            clipboard.get_text()
        } else {
            None
        }
    }
}
