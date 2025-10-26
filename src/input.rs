use crate::dpi::LogicalPoint;
use ahash::{HashMap, HashSet};
use std::time::{Duration, Instant};
use winit::{
    event::MouseButton,
    keyboard::{KeyCode, ModifiersState},
};

#[derive(Default)]
pub struct InputState {
    pub key_modifiers: ModifiersState,
    keys_pressed: HashSet<KeyCode>,
    mouse_pressed: HashSet<MouseButton>,
    mouse_last_pressed: HashMap<MouseButton, Instant>,
    cursor_pos: LogicalPoint<f32>,
}

impl InputState {
    pub fn set_key_pressed(&mut self, code: KeyCode, is_pressed: bool) {
        if is_pressed {
            self.keys_pressed.insert(code);
        } else {
            self.keys_pressed.remove(&code);
        }
    }

    pub fn key_pressed(&self, code: KeyCode) -> bool {
        self.keys_pressed.contains(&code)
    }

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

    pub fn mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse_pressed.contains(&button)
    }

    pub fn mouse_pos(&self) -> LogicalPoint<f32> {
        self.cursor_pos
    }

    pub fn set_mouse_pos(&mut self, pos: LogicalPoint<f32>) {
        self.cursor_pos = pos;
    }
}

pub fn str_as_keycode(s: &str) -> Option<KeyCode> {
    Some(match s.to_uppercase().as_str() {
        // Letters
        "A" => KeyCode::KeyA,
        "B" => KeyCode::KeyB,
        "C" => KeyCode::KeyC,
        "D" => KeyCode::KeyD,
        "E" => KeyCode::KeyE,
        "F" => KeyCode::KeyF,
        "G" => KeyCode::KeyG,
        "H" => KeyCode::KeyH,
        "I" => KeyCode::KeyI,
        "J" => KeyCode::KeyJ,
        "K" => KeyCode::KeyK,
        "L" => KeyCode::KeyL,
        "M" => KeyCode::KeyM,
        "N" => KeyCode::KeyN,
        "O" => KeyCode::KeyO,
        "P" => KeyCode::KeyP,
        "Q" => KeyCode::KeyQ,
        "R" => KeyCode::KeyR,
        "S" => KeyCode::KeyS,
        "T" => KeyCode::KeyT,
        "U" => KeyCode::KeyU,
        "V" => KeyCode::KeyV,
        "W" => KeyCode::KeyW,
        "X" => KeyCode::KeyX,
        "Y" => KeyCode::KeyY,
        "Z" => KeyCode::KeyZ,

        // Digits
        "0" => KeyCode::Digit0,
        "1" => KeyCode::Digit1,
        "2" => KeyCode::Digit2,
        "3" => KeyCode::Digit3,
        "4" => KeyCode::Digit4,
        "5" => KeyCode::Digit5,
        "6" => KeyCode::Digit6,
        "7" => KeyCode::Digit7,
        "8" => KeyCode::Digit8,
        "9" => KeyCode::Digit9,

        // Modifiers
        "SHIFT" => KeyCode::ShiftLeft,
        "CTRL" => KeyCode::ControlLeft,
        "ALT" => KeyCode::AltLeft,

        // F Keys
        "F1" => KeyCode::F1,
        "F2" => KeyCode::F2,
        "F3" => KeyCode::F3,
        "F4" => KeyCode::F4,
        "F5" => KeyCode::F5,
        "F6" => KeyCode::F6,
        "F7" => KeyCode::F7,
        "F8" => KeyCode::F8,
        "F9" => KeyCode::F9,
        "F10" => KeyCode::F10,
        "F11" => KeyCode::F11,
        "F12" => KeyCode::F12,

        // Rest
        " " => KeyCode::Space,
        "BACK" => KeyCode::Backspace,
        "TAB" => KeyCode::Tab,
        "RETURN" => KeyCode::Enter,
        "ESCAPE" => KeyCode::Escape,
        "PAUSE" => KeyCode::Pause,
        "PAGEUP" => KeyCode::PageUp,
        "PAGEDOWN" => KeyCode::PageDown,
        "END" => KeyCode::End,
        "HOME" => KeyCode::Home,
        "PRINTSCREEN" => KeyCode::PrintScreen,
        "INSERT" => KeyCode::Insert,
        "DELETE" => KeyCode::Delete,
        "UP" => KeyCode::ArrowUp,
        "DOWN" => KeyCode::ArrowDown,
        "LEFT" => KeyCode::ArrowLeft,
        "RIGHT" => KeyCode::ArrowRight,
        "NUMLOCK" => KeyCode::NumLock,
        "SCROLL" => KeyCode::ScrollLock,

        _ => return None,
    })
}

pub fn keycode_as_str(code: KeyCode) -> Option<&'static str> {
    Some(match code {
        // Letters
        KeyCode::KeyA => "a",
        KeyCode::KeyB => "b",
        KeyCode::KeyC => "c",
        KeyCode::KeyD => "d",
        KeyCode::KeyE => "e",
        KeyCode::KeyF => "f",
        KeyCode::KeyG => "g",
        KeyCode::KeyH => "h",
        KeyCode::KeyI => "i",
        KeyCode::KeyJ => "j",
        KeyCode::KeyK => "k",
        KeyCode::KeyL => "l",
        KeyCode::KeyM => "m",
        KeyCode::KeyN => "n",
        KeyCode::KeyO => "o",
        KeyCode::KeyP => "p",
        KeyCode::KeyQ => "q",
        KeyCode::KeyR => "r",
        KeyCode::KeyS => "s",
        KeyCode::KeyT => "t",
        KeyCode::KeyU => "u",
        KeyCode::KeyV => "v",
        KeyCode::KeyW => "w",
        KeyCode::KeyX => "x",
        KeyCode::KeyY => "y",
        KeyCode::KeyZ => "z",

        // Digits
        KeyCode::Digit0 => "0",
        KeyCode::Digit1 => "1",
        KeyCode::Digit2 => "2",
        KeyCode::Digit3 => "3",
        KeyCode::Digit4 => "4",
        KeyCode::Digit5 => "5",
        KeyCode::Digit6 => "6",
        KeyCode::Digit7 => "7",
        KeyCode::Digit8 => "8",
        KeyCode::Digit9 => "9",

        // Modifiers
        KeyCode::ShiftLeft => "SHIFT",
        KeyCode::ControlLeft => "CTRL",
        KeyCode::AltLeft => "ALT",

        // F Keys
        KeyCode::F1 => "F1",
        KeyCode::F2 => "F2",
        KeyCode::F3 => "F3",
        KeyCode::F4 => "F4",
        KeyCode::F5 => "F5",
        KeyCode::F6 => "F6",
        KeyCode::F7 => "F7",
        KeyCode::F8 => "F8",
        KeyCode::F9 => "F9",
        KeyCode::F10 => "F10",
        KeyCode::F11 => "F11",
        KeyCode::F12 => "F12",

        // Rest
        KeyCode::Space => " ",
        KeyCode::Backspace => "BACK",
        KeyCode::Tab => "TAB",
        KeyCode::Enter => "RETURN",
        KeyCode::Escape => "ESCAPE",
        KeyCode::Pause => "PAUSE",
        KeyCode::PageUp => "PAGEUP",
        KeyCode::PageDown => "PAGEDOWN",
        KeyCode::End => "END",
        KeyCode::Home => "HOME",
        KeyCode::PrintScreen => "PRINTSCREEN",
        KeyCode::Insert => "INSERT",
        KeyCode::Delete => "DELETE",
        KeyCode::ArrowUp => "UP",
        KeyCode::ArrowDown => "DOWN",
        KeyCode::ArrowLeft => "LEFT",
        KeyCode::ArrowRight => "RIGHT",
        KeyCode::NumLock => "NUMLOCK",
        KeyCode::ScrollLock => "SCROLL",

        KeyCode::Equal => "+", // This is what PoB does
        KeyCode::Minus => "-",
        KeyCode::Comma => ",",
        KeyCode::Period => ".",
        KeyCode::Slash => "/",

        KeyCode::NumpadAdd => "+",
        KeyCode::NumpadSubtract => "-",
        KeyCode::NumpadEnter => "RETURN",
        KeyCode::Numpad0 => "0",

        _ => return None,
    })
}

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

pub fn mousebutton_as_str(button: MouseButton) -> Option<&'static str> {
    Some(match button {
        MouseButton::Left => "LEFTBUTTON",
        MouseButton::Right => "RIGHTBUTTON",
        MouseButton::Middle => "MIDDLEBUTTON",
        MouseButton::Back => "MOUSE4",
        MouseButton::Forward => "MOUSE5",
        _ => return None,
    })
}
