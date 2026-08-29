//! Module to handle user inputs like keyboard keys and mouse buttons.

use crate::dpi::LogicalPoint;
use ahash::{HashMap, HashSet};
use std::time::{Duration, Instant};

pub type Key = winit::keyboard::Key;
pub type MouseButton = winit::event::MouseButton;
pub type ModifiersState = winit::keyboard::ModifiersState;

/// Max gap between two clicks for them to count as a double-click.
const DOUBLE_CLICK_DURATION: Duration = Duration::from_millis(500);
/// Max cursor movement between two clicks for them to count as a double-click.
const DOUBLE_CLICK_MOVE_THRESHOLD: f32 = 5.0;

/// When and where a mouse button was last pressed, for double-click detection.
#[derive(Debug, Clone)]
struct MousePressState {
    time: Instant,
    pos: LogicalPoint<f32>,
}

/// Tracks currently-pressed keys/buttons and cursor position.
#[derive(Default)]
pub struct InputState {
    /// Currently held modifier keys (Shift/Ctrl/Alt/Super).
    pub key_modifiers: ModifiersState,
    /// Keys currently held down.
    keys_pressed: HashSet<Key>,
    /// Mouse buttons currently held down.
    mouse_pressed: HashSet<MouseButton>,
    /// Last press time/position per mouse button, for double-click detection.
    mouse_last_pressed: HashMap<MouseButton, MousePressState>,
    /// Current mouse cursor position.
    cursor_pos: LogicalPoint<f32>,
}

impl InputState {
    /// Marks 'key' as pressed or released.
    pub fn set_key_pressed(&mut self, key: &Key, is_pressed: bool) {
        if is_pressed {
            self.keys_pressed.insert(key.clone());
        } else {
            self.keys_pressed.remove(key);
        }
    }

    /// Returns whether `key` is currently pressed.
    pub fn key_pressed(&self, key: &Key) -> bool {
        self.keys_pressed.contains(key)
    }

    /// Marks `button` as pressed or released.
    /// Returns `true` if this press counts as a double-click (always `false` on release).
    pub fn set_mouse_pressed(&mut self, button: MouseButton, is_pressed: bool) -> bool {
        if !is_pressed {
            self.mouse_pressed.remove(&button);
            return false;
        }
        self.mouse_pressed.insert(button);
        let now = Instant::now();
        let pos = self.cursor_pos;
        match self.mouse_last_pressed.entry(button) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let last = entry.get();
                let delta = (last.pos - pos).abs();
                let is_double = now.duration_since(last.time) < DOUBLE_CLICK_DURATION
                    && delta.x <= DOUBLE_CLICK_MOVE_THRESHOLD
                    && delta.y <= DOUBLE_CLICK_MOVE_THRESHOLD;
                if is_double {
                    // Reset so that a triple-click is not treated as two consecutive
                    // double-clicks; the next click starts a fresh timing window.
                    entry.remove();
                } else {
                    entry.insert(MousePressState { time: now, pos });
                }
                is_double
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(MousePressState { time: now, pos });
                false
            }
        }
    }

    /// Returns whether `button` is currently pressed.
    pub fn mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse_pressed.contains(&button)
    }

    /// Returns the last known cursor position.
    pub fn mouse_pos(&self) -> LogicalPoint<f32> {
        self.cursor_pos
    }

    /// Updates the cursor position.
    pub fn set_mouse_pos(&mut self, pos: LogicalPoint<f32>) {
        self.cursor_pos = pos;
    }

    /// Clears all pressed keys, buttons, and modifier states.
    pub fn clear_pressed(&mut self) {
        self.keys_pressed.clear();
        self.mouse_pressed.clear();
        self.mouse_last_pressed.clear();
        self.key_modifiers = ModifiersState::empty();
    }
}
