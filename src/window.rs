use crate::{
    clipboard::Clipboard,
    dpi::{LogicalSize, PhysicalSize, ScaleFactor},
};
use raw_window_handle::HasDisplayHandle;
use std::sync::Arc;
use winit::window::Window;

pub struct WindowState {
    // NOTE: clipboard needs to be destroyed before window
    clipboard: Option<Clipboard>,
    pub window: Option<Arc<Window>>,
    pub size: PhysicalSize<u32>,
    scale_factor: ScaleFactor<f32>,
    scale_factor_override: Option<ScaleFactor<f32>>,
    pending_window_title: std::cell::Cell<Option<String>>,
    pub is_hovered: bool,
    pub is_focused: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            window: None,
            size: Default::default(),
            scale_factor: ScaleFactor::identity(),
            scale_factor_override: None,
            pending_window_title: std::cell::Cell::new(None),
            clipboard: None,
            is_hovered: true,
            is_focused: true,
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
        self.scale_factor = ScaleFactor::new(window.scale_factor() as f32);

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

    pub fn logical_size(&self) -> LogicalSize<f32> {
        self.size.cast() / self.scale_factor()
    }

    pub fn scale_factor(&self) -> ScaleFactor<f32> {
        self.scale_factor_override.unwrap_or(self.scale_factor)
    }

    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        self.scale_factor = ScaleFactor::new(scale_factor);
    }

    pub fn scale_factor_override(&self) -> Option<f32> {
        self.scale_factor_override.map(|s| s.get())
    }

    pub fn set_scale_factor_override(&mut self, scale_factor: Option<f32>) {
        self.scale_factor_override = scale_factor.map(ScaleFactor::new);
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
        self.clipboard.as_mut().and_then(Clipboard::get_text)
    }
}
