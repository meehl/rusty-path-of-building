use crate::lua::ContextSocket;
use mlua::{Lua, Result as LuaResult};

// NOTE: needs to return physical size if this MR gets merged:
// https://github.com/PathOfBuildingCommunity/PathOfBuilding-PoE2/pull/1420
pub fn get_screen_size(l: &Lua, _: ()) -> LuaResult<(u32, u32)> {
    let socket = l.app_data_ref::<&'static ContextSocket>().unwrap();
    let size = socket.window().logical_size();
    Ok((size.width, size.height))
}

pub fn get_screen_scale(l: &Lua, _: ()) -> LuaResult<f32> {
    let socket = l.app_data_ref::<&'static ContextSocket>().unwrap();
    Ok(socket.window().scale_factor)
}

pub fn set_window_title(l: &Lua, title: String) -> LuaResult<()> {
    let socket = l.app_data_ref::<&'static ContextSocket>().unwrap();
    socket.window().set_window_title(&title);
    Ok(())
}
