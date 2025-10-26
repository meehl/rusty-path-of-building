use crate::{
    input::{str_as_keycode, str_as_mousebutton},
    lua::ContextSocket,
};
use mlua::{Lua, Result as LuaResult};

pub fn get_cursor_pos(l: &Lua, _: ()) -> LuaResult<(u32, u32)> {
    let socket = l.app_data_ref::<&'static ContextSocket>().unwrap();
    let pos = socket.input().mouse_pos();
    Ok((pos.x as u32, pos.y as u32))
}

pub fn is_key_down(l: &Lua, key_name: String) -> LuaResult<bool> {
    let socket = l.app_data_ref::<&'static ContextSocket>().unwrap();

    if let Some(code) = str_as_keycode(&key_name) {
        Ok(socket.input().key_pressed(code))
    } else if let Some(button) = str_as_mousebutton(&key_name) {
        Ok(socket.input().mouse_pressed(button))
    } else {
        Ok(false)
    }
}
