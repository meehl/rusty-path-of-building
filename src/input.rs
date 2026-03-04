//! Module to handle user inputs like keyboard keys and mouse buttons.

use crate::dpi::LogicalPoint;
use ahash::{HashMap, HashSet};
use std::time::{Duration, Instant};
use winit::{
    event::MouseButton,
    keyboard::{Key, ModifiersState, NamedKey, SmolStr},
};

/// Current state of various keyboard and mouse inputs for the application.
#[derive(Default)]
pub struct InputState {
    /// Current state(s) of modifier keys. (Shift, Control, Alt, Super)
    pub key_modifiers: ModifiersState,
    /// HashSet of currently pressed keyboard keys.
    keys_pressed: HashSet<Key>,
    /// HashSet of currently pressed mouse buttons.
    mouse_pressed: HashSet<MouseButton>,
    /// HashMap of mouse buttons (keys) with the last time they were pressed.
    mouse_last_pressed: HashMap<MouseButton, Instant>,
    /// Current cursor position relative to the top-left corner of the window.
    cursor_pos: LogicalPoint<f32>,
}

impl InputState {
    /// Updates [`Self::keys_pressed`] based on `is_pressed`.
    pub fn set_key_pressed(&mut self, key: Key, is_pressed: bool) {
        if is_pressed {
            self.keys_pressed.insert(key);
        } else {
            self.keys_pressed.remove(&key);
        }
    }

    /// Returns if the key is pressed (`true`) or not pressed (`false`).
    pub fn key_pressed(&self, key: Key) -> bool {
        self.keys_pressed.contains(&key)
    }

    /// Updates [`Self::mouse_pressed`](field@Self::mouse_pressed) based on provided
    /// `button` and `is_pressed`.
    pub fn set_mouse_pressed(&mut self, button: MouseButton, is_pressed: bool) -> bool {
        if is_pressed {
            self.mouse_pressed.insert(button);
        } else {
            self.mouse_pressed.remove(&button);
        }

        let now = Instant::now();
        let last = self.mouse_last_pressed.entry(button);

        match last {
            std::collections::hash_map::Entry::Occupied(mut occupied_entry) => {
                let last = occupied_entry.insert(now);
                now.duration_since(last) < Duration::from_millis(400)
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(now);
                false
            }
        }
    }

    /// Returns `true` if the `button` was pressed and no release has been seen.
    pub fn mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse_pressed.contains(&button)
    }

    /// Returns the last known cursor position.
    pub fn mouse_pos(&self) -> LogicalPoint<f32> {
        self.cursor_pos
    }

    /// Sets [`Self::cursor_pos`] to the provided `pos`.
    pub fn set_mouse_pos(&mut self, pos: LogicalPoint<f32>) {
        self.cursor_pos = pos;
    }

    /// Clears all pressed keys, buttons, and modifier states. Used when the
    /// application loses focus to avoid keys being "stuck" on/pressed.
    pub fn clear_pressed(&mut self) {
        self.keys_pressed.clear();
        self.mouse_pressed.clear();
        self.key_modifiers = ModifiersState::empty();
    }
}

/// Attempts to convert the provided string `s` to a [winit::keyboard::Key].
/// Returns [None] if no matching Key found.
pub fn str_as_key(s: &str) -> Option<Key> {
    Some(match s.to_uppercase().as_str() {
        "A" => Key::Character(SmolStr::new_static("A")),
        "B" => Key::Character(SmolStr::new_static("B")),
        "C" => Key::Character(SmolStr::new_static("C")),
        "D" => Key::Character(SmolStr::new_static("D")),
        "E" => Key::Character(SmolStr::new_static("E")),
        "F" => Key::Character(SmolStr::new_static("F")),
        "G" => Key::Character(SmolStr::new_static("G")),
        "H" => Key::Character(SmolStr::new_static("H")),
        "I" => Key::Character(SmolStr::new_static("I")),
        "J" => Key::Character(SmolStr::new_static("J")),
        "K" => Key::Character(SmolStr::new_static("K")),
        "L" => Key::Character(SmolStr::new_static("L")),
        "M" => Key::Character(SmolStr::new_static("M")),
        "N" => Key::Character(SmolStr::new_static("N")),
        "O" => Key::Character(SmolStr::new_static("O")),
        "P" => Key::Character(SmolStr::new_static("P")),
        "Q" => Key::Character(SmolStr::new_static("Q")),
        "R" => Key::Character(SmolStr::new_static("R")),
        "S" => Key::Character(SmolStr::new_static("S")),
        "T" => Key::Character(SmolStr::new_static("T")),
        "U" => Key::Character(SmolStr::new_static("U")),
        "V" => Key::Character(SmolStr::new_static("V")),
        "W" => Key::Character(SmolStr::new_static("W")),
        "X" => Key::Character(SmolStr::new_static("X")),
        "Y" => Key::Character(SmolStr::new_static("Y")),
        "Z" => Key::Character(SmolStr::new_static("Z")),

        "0" => Key::Character(SmolStr::new_static("0")),
        "1" => Key::Character(SmolStr::new_static("1")),
        "2" => Key::Character(SmolStr::new_static("2")),
        "3" => Key::Character(SmolStr::new_static("3")),
        "4" => Key::Character(SmolStr::new_static("4")),
        "5" => Key::Character(SmolStr::new_static("5")),
        "6" => Key::Character(SmolStr::new_static("6")),
        "7" => Key::Character(SmolStr::new_static("7")),
        "8" => Key::Character(SmolStr::new_static("8")),
        "9" => Key::Character(SmolStr::new_static("9")),

        "SHIFT" => Key::Named(NamedKey::Shift),
        "CTRL" => Key::Named(NamedKey::Control),
        "ALT" => Key::Named(NamedKey::Alt),

        "F1" => Key::Named(NamedKey::F1),
        "F2" => Key::Named(NamedKey::F2),
        "F3" => Key::Named(NamedKey::F3),
        "F4" => Key::Named(NamedKey::F4),
        "F5" => Key::Named(NamedKey::F5),
        "F6" => Key::Named(NamedKey::F6),
        "F7" => Key::Named(NamedKey::F7),
        "F8" => Key::Named(NamedKey::F8),
        "F9" => Key::Named(NamedKey::F9),
        "F10" => Key::Named(NamedKey::F10),
        "F11" => Key::Named(NamedKey::F11),
        "F12" => Key::Named(NamedKey::F12),

        " " => Key::Named(NamedKey::Space),
        "BACK" => Key::Named(NamedKey::Backspace),
        "TAB" => Key::Named(NamedKey::Tab),
        "RETURN" => Key::Named(NamedKey::Enter),
        "ESCAPE" => Key::Named(NamedKey::Escape),
        "PAUSE" => Key::Named(NamedKey::Pause),
        "PAGEUP" => Key::Named(NamedKey::PageUp),
        "PAGEDOWN" => Key::Named(NamedKey::PageDown),
        "END" => Key::Named(NamedKey::End),
        "HOME" => Key::Named(NamedKey::Home),
        "PRINTSCREEN" => Key::Named(NamedKey::PrintScreen),
        "INSERT" => Key::Named(NamedKey::Insert),
        "DELETE" => Key::Named(NamedKey::Delete),
        "UP" => Key::Named(NamedKey::ArrowUp),
        "DOWN" => Key::Named(NamedKey::ArrowDown),
        "LEFT" => Key::Named(NamedKey::ArrowLeft),
        "RIGHT" => Key::Named(NamedKey::ArrowRight),
        "NUMLOCK" => Key::Named(NamedKey::NumLock),
        "SCROLL" => Key::Named(NamedKey::ScrollLock),

        _ => return None,
    })
}

/// Attempts to convert the provided [winit::keyboard::Key] `key` to a string
/// representation that PoB recognizes.
///
/// Returns [None] if no matching string found.
pub fn key_as_str(key: Key) -> Option<SmolStr> {
    Some(match key {
        Key::Character(ch) => {
            if ch == "=" {
                SmolStr::new("+") // This is what PoB does
            } else {
                ch
            }
        }
        Key::Named(named) => SmolStr::new(match named {
            NamedKey::Shift => "SHIFT",
            NamedKey::Control => "CTRL",
            NamedKey::Alt => "ALT",
            NamedKey::F1 => "F1",
            NamedKey::F2 => "F2",
            NamedKey::F3 => "F3",
            NamedKey::F4 => "F4",
            NamedKey::F5 => "F5",
            NamedKey::F6 => "F6",
            NamedKey::F7 => "F7",
            NamedKey::F8 => "F8",
            NamedKey::F9 => "F9",
            NamedKey::F10 => "F10",
            NamedKey::F11 => "F11",
            NamedKey::F12 => "F12",
            NamedKey::Space => " ",
            NamedKey::Backspace => "BACK",
            NamedKey::Tab => "TAB",
            NamedKey::Enter => "RETURN",
            NamedKey::Escape => "ESCAPE",
            NamedKey::Pause => "PAUSE",
            NamedKey::PageUp => "PAGEUP",
            NamedKey::PageDown => "PAGEDOWN",
            NamedKey::End => "END",
            NamedKey::Home => "HOME",
            NamedKey::PrintScreen => "PRINTSCREEN",
            NamedKey::Insert => "INSERT",
            NamedKey::Delete => "DELETE",
            NamedKey::ArrowUp => "UP",
            NamedKey::ArrowDown => "DOWN",
            NamedKey::ArrowLeft => "LEFT",
            NamedKey::ArrowRight => "RIGHT",
            NamedKey::NumLock => "NUMLOCK",
            NamedKey::ScrollLock => "SCROLL",
            _ => return None,
        }),
        _ => return None,
    })
}

/// Attempts to convert the provided [&str] `s` from the PoB Lua Backend to a
/// [MouseButton].
///
/// Returns [None] if no matching string found.
pub fn str_as_mousebutton(s: &str) -> Option<MouseButton> {
    Some(match s.to_uppercase().as_str() {
        "LEFTBUTTON" => MouseButton::Left,
        "RIGHTBUTTON" => MouseButton::Right,
        "MIDDLEBUTTON" => MouseButton::Middle,
        "MOUSE4" => MouseButton::Back,
        "MOUSE5" => MouseButton::Forward,
        _ => return None,
    })
}

/// Attempts to convert the provided [MouseButton] to a [SmolStr] that the PoB Lua
/// backend recognizes.
///
/// Returns [None] if no matching [MouseButton] was found.
pub fn mousebutton_as_str(button: MouseButton) -> Option<SmolStr> {
    Some(match button {
        MouseButton::Left => SmolStr::new("LEFTBUTTON"),
        MouseButton::Right => SmolStr::new("RIGHTBUTTON"),
        MouseButton::Middle => SmolStr::new("MIDDLEBUTTON"),
        MouseButton::Back => SmolStr::new("MOUSE4"),
        MouseButton::Forward => SmolStr::new("MOUSE5"),
        _ => return None,
    })
}
