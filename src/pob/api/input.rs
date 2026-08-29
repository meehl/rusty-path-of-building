use crate::{
    input::{str_as_key, str_as_mousebutton},
    pob::Context,
};
use mlua::{Lua, Result as LuaResult};

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
