use raw_window_handle::RawDisplayHandle;

/// Abstraction over clipboard crates
///
/// `smithay_clipboard`: Used on wayland
/// `arboard`: All other platforms (check arboard docs for platform support)
pub struct Clipboard {
    #[cfg(target_family = "unix")]
    smithay: Option<smithay_clipboard::Clipboard>,
    arboard: Option<arboard::Clipboard>,
    // fallback if everything else fails. only supports intra-application copy/paste
    fallback: Option<String>,
}

impl Clipboard {
    pub fn new(_raw_display_handle: Option<RawDisplayHandle>) -> Self {
        Self {
            #[cfg(target_family = "unix")]
            smithay: create_smithay_clipboard(_raw_display_handle),
            arboard: create_arboard_clipboard(),
            fallback: None,
        }
    }

    /// Sets the text content of clipboard
    pub fn set_text(&mut self, text: String) {
        #[cfg(target_family = "unix")]
        if let Some(clipboard) = &mut self.smithay {
            clipboard.store(text);
            return;
        }

        if let Some(clipboard) = &mut self.arboard {
            let _ = clipboard.set_text(text);
            return;
        }

        self.fallback = Some(text);
    }

    /// Gets the text content of clipboard
    pub fn get_text(&mut self) -> Option<String> {
        #[cfg(target_family = "unix")]
        if let Some(clipboard) = &mut self.smithay {
            return clipboard.load().ok();
        }

        if let Some(clipboard) = &mut self.arboard {
            return clipboard.get_text().ok();
        }

        if let Some(text) = &self.fallback {
            return Some(text.clone());
        }

        None
    }
}

#[cfg(target_family = "unix")]
fn create_smithay_clipboard(
    raw_display_handle: Option<RawDisplayHandle>,
) -> Option<smithay_clipboard::Clipboard> {
    if let Some(RawDisplayHandle::Wayland(handle)) = raw_display_handle {
        log::debug!("Creating smithay clipboard...");
        Some(unsafe { smithay_clipboard::Clipboard::new(handle.display.as_ptr()) })
    } else {
        None
    }
}

fn create_arboard_clipboard() -> Option<arboard::Clipboard> {
    log::debug!("Creating arboard clipboard...");
    match arboard::Clipboard::new() {
        Ok(clipboard) => Some(clipboard),
        Err(err) => {
            log::warn!("Failed to create arboard clipboard: {err}");
            None
        }
    }
}
