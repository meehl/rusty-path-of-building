use crate::{
    input::{Key, MouseButton},
    pob::Context,
};
use mlua::{Lua, Result as LuaResult};
use winit::keyboard::{NamedKey, SmolStr};

pub fn get_cursor_pos(l: &Lua, _: ()) -> LuaResult<(u32, u32)> {
    let ctx = l.app_data_ref::<Context>().unwrap();
    let pos = ctx.input_state.mouse_pos();
    Ok((pos.x as u32, pos.y as u32))
}

pub fn set_cursor_pos(_l: &Lua, _: ()) -> LuaResult<()> {
    unimplemented!()
}

pub fn show_cursor(_l: &Lua, _: ()) -> LuaResult<()> {
    unimplemented!()
}

pub fn is_key_down(l: &Lua, key_name: String) -> LuaResult<bool> {
    let ctx = l.app_data_ref::<Context>().unwrap();

    if let Some(key) = str_as_key(&key_name) {
        Ok(ctx.input_state.key_pressed(&key))
    } else if let Some(button) = str_as_mousebutton(&key_name) {
        Ok(ctx.input_state.mouse_pressed(button))
    } else {
        Ok(false)
    }
}

const NAMED_KEYS: &[(&str, NamedKey)] = &[
    ("BACK", NamedKey::Backspace),
    ("TAB", NamedKey::Tab),
    ("RETURN", NamedKey::Enter),
    ("ESCAPE", NamedKey::Escape),
    ("SHIFT", NamedKey::Shift),
    ("CTRL", NamedKey::Control),
    ("ALT", NamedKey::Alt),
    ("PAUSE", NamedKey::Pause),
    ("PAGEUP", NamedKey::PageUp),
    ("PAGEDOWN", NamedKey::PageDown),
    ("END", NamedKey::End),
    ("HOME", NamedKey::Home),
    ("PRINTSCREEN", NamedKey::PrintScreen),
    ("INSERT", NamedKey::Insert),
    ("DELETE", NamedKey::Delete),
    ("UP", NamedKey::ArrowUp),
    ("DOWN", NamedKey::ArrowDown),
    ("LEFT", NamedKey::ArrowLeft),
    ("RIGHT", NamedKey::ArrowRight),
    ("F1", NamedKey::F1),
    ("F2", NamedKey::F2),
    ("F3", NamedKey::F3),
    ("F4", NamedKey::F4),
    ("F5", NamedKey::F5),
    ("F6", NamedKey::F6),
    ("F7", NamedKey::F7),
    ("F8", NamedKey::F8),
    ("F9", NamedKey::F9),
    ("F10", NamedKey::F10),
    ("F11", NamedKey::F11),
    ("F12", NamedKey::F12),
    ("F13", NamedKey::F13),
    ("F14", NamedKey::F14),
    ("F15", NamedKey::F15),
    (" ", NamedKey::Space),
    ("NUMLOCK", NamedKey::NumLock),
    ("SCROLL", NamedKey::ScrollLock),
];

/// Atempts to convert PoB's key representation into a `Key`.
fn str_as_key(s: &str) -> Option<Key> {
    let upper = s.to_uppercase();

    // named keys
    if let Some((_, named)) = NAMED_KEYS.iter().find(|(name, _)| *name == upper) {
        return Some(Key::Named(*named));
    }

    // single-character keys
    let mut chars = upper.chars();
    let first = chars.next()?;
    chars
        .next()
        .is_none()
        .then(|| Key::Character(SmolStr::new(first.to_string())))
}

/// Atempts to convert a `Key` into PoB's key representation
pub fn key_as_str(key: Key) -> Option<SmolStr> {
    match key {
        Key::Character(ch) if ch == "=" => Some(SmolStr::new("+")), // matches PoB's behavior
        Key::Character(ch) => Some(ch),
        Key::Named(named) => NAMED_KEYS
            .iter()
            .find(|(_, n)| *n == named)
            .map(|(name, _)| SmolStr::new(*name)),
        _ => None,
    }
}

const MOUSE_BUTTONS: &[(&str, MouseButton)] = &[
    ("LEFTBUTTON", MouseButton::Left),
    ("RIGHTBUTTON", MouseButton::Right),
    ("MIDDLEBUTTON", MouseButton::Middle),
    ("MOUSE4", MouseButton::Back),
    ("MOUSE5", MouseButton::Forward),
];

/// Atempts to convert PoB's key representation into a `MouseButton`.
fn str_as_mousebutton(s: &str) -> Option<MouseButton> {
    MOUSE_BUTTONS
        .iter()
        .find(|(name, _)| *name == s.to_uppercase())
        .map(|(_, button)| *button)
}

/// Atempts to convert a `MouseButton` into PoB's key representation
pub fn mousebutton_as_str(button: MouseButton) -> Option<SmolStr> {
    MOUSE_BUTTONS
        .iter()
        .find(|(_, b)| *b == button)
        .map(|(name, _)| SmolStr::new(*name))
}
