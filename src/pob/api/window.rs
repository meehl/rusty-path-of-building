use crate::{
    dpi::{LogicalSize, PhysicalSize},
    pob::Context,
};
use mlua::{Lua, Result as LuaResult};

pub fn get_screen_size(l: &Lua, _: ()) -> LuaResult<(u32, u32)> {
    let ctx = l.app_data_ref::<Context>().unwrap();
    let size = if ctx.is_dpi_aware {
        let PhysicalSize { width, height, .. } = ctx.window_state.size;
        (width, height)
    } else {
        let LogicalSize { width, height, .. } = ctx.window_state.logical_size().cast();
        (width, height)
    };
    Ok(size)
}

pub fn get_screen_scale(l: &Lua, _: ()) -> LuaResult<f32> {
    let ctx = l.app_data_ref::<Context>().unwrap();
    let scale_factor = ctx.window_state.scale_factor();
    Ok(scale_factor.get())
}

pub fn set_window_title(l: &Lua, title: String) -> LuaResult<()> {
    let ctx = l.app_data_ref::<Context>().unwrap();
    ctx.window_state.set_window_title(&title);
    Ok(())
}

pub fn set_foreground(l: &Lua, _: ()) -> LuaResult<()> {
    let ctx = l.app_data_ref::<Context>().unwrap();
    ctx.window_state.focus();
    Ok(())
}

pub fn set_dpi_scale_override(l: &Lua, percent: i32) -> LuaResult<()> {
    let mut ctx = l.app_data_mut::<Context>().unwrap();
    match percent {
        0 => ctx.window_state.set_scale_factor_override(None),
        p if p > 0 => ctx
            .window_state
            .set_scale_factor_override(Some(p as f32 / 100.0)),
        _ => {}
    }
    Ok(())
}

pub fn get_dpi_scale_override(l: &Lua, _: ()) -> LuaResult<i32> {
    let ctx = l.app_data_ref::<Context>().unwrap();
    match ctx.window_state.scale_factor_override() {
        Some(scale_factor) => Ok((scale_factor * 100.0) as i32),
        None => Ok(0),
    }
}
