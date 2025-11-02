use crate::{
    dpi::{LogicalSize, PhysicalSize},
    lua::Context,
};
use mlua::{Lua, Result as LuaResult};

pub fn get_screen_size(l: &Lua, _: ()) -> LuaResult<(u32, u32)> {
    let ctx = l.app_data_ref::<&'static Context>().unwrap();
    let size = if *ctx.is_dpi_aware() {
        let PhysicalSize { width, height, .. } = ctx.window().size;
        (width, height)
    } else {
        let LogicalSize { width, height, .. } = ctx.window().logical_size();
        (width, height)
    };
    Ok(size)
}

pub fn get_screen_scale(l: &Lua, _: ()) -> LuaResult<f32> {
    let ctx = l.app_data_ref::<&'static Context>().unwrap();
    let scale_factor = ctx.window().scale_factor();
    Ok(scale_factor)
}

pub fn set_window_title(l: &Lua, title: String) -> LuaResult<()> {
    let ctx = l.app_data_ref::<&'static Context>().unwrap();
    ctx.window().set_window_title(&title);
    Ok(())
}

pub fn set_foreground(l: &Lua, _: ()) -> LuaResult<()> {
    let ctx = l.app_data_ref::<&'static Context>().unwrap();
    ctx.window().focus();
    Ok(())
}

pub fn set_dpi_scale_override(l: &Lua, percent: i32) -> LuaResult<()> {
    let socket = l.app_data_ref::<&'static Context>().unwrap();
    match percent {
        0 => socket.window().scale_factor_override = None,
        p if p > 0 => socket.window().scale_factor_override = Some(p as f32 / 100.0),
        _ => {}
    }
    Ok(())
}

pub fn get_dpi_scale_override(l: &Lua, _: ()) -> LuaResult<i32> {
    let socket = l.app_data_ref::<&'static Context>().unwrap();
    match socket.window().scale_factor_override {
        Some(scale_factor) => Ok((scale_factor * 100.0) as i32),
        None => Ok(0),
    }
}
