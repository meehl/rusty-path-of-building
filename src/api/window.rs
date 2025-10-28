use crate::{
    dpi::{LogicalSize, PhysicalSize},
    lua::ContextSocket,
};
use mlua::{Lua, Result as LuaResult};

pub fn get_screen_size(l: &Lua, _: ()) -> LuaResult<(u32, u32)> {
    let socket = l.app_data_ref::<&'static ContextSocket>().unwrap();
    let size = if *socket.is_dpi_aware() {
        let PhysicalSize { width, height, .. } = socket.window().size;
        (width, height)
    } else {
        let LogicalSize { width, height, .. } = socket.window().logical_size();
        (width, height)
    };
    Ok(size)
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
