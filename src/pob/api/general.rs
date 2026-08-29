use crate::pob::{Context, api::rendering::PoBString};
use mlua::{IntoLuaMulti, Lua, MultiValue, Result as LuaResult, Variadic};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn get_time(_l: &Lua, _: ()) -> LuaResult<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis())
}

pub fn strip_escapes(_: &Lua, text: String) -> LuaResult<String> {
    Ok(PoBString(&text).strip_escapes())
}

pub fn exit(l: &Lua, exit_msg: Option<String>) -> LuaResult<()> {
    if let Some(exit_msg) = exit_msg {
        println!("{exit_msg}");
    }
    let mut ctx = l.app_data_mut::<Context>().unwrap();
    ctx.should_exit = true;
    Ok(())
}

pub fn restart(l: &Lua, _: ()) -> LuaResult<()> {
    let mut ctx = l.app_data_mut::<Context>().unwrap();
    ctx.needs_restart = true;
    Ok(())
}

pub fn open_url(l: &Lua, url: String) -> LuaResult<MultiValue> {
    match open::that(url) {
        Ok(()) => ().into_lua_multi(l),
        Err(_) => "Unable to open url!".into_lua_multi(l),
    }
}

pub fn render_init(l: &Lua, features: Variadic<String>) -> LuaResult<()> {
    let mut ctx = l.app_data_mut::<Context>().unwrap();
    ctx.is_dpi_aware = features.iter().any(|feat| feat == "DPI_AWARE");
    Ok(())
}

pub fn take_screenshot(_l: &Lua, _: ()) -> LuaResult<()> {
    log::warn!("take_screenshot is not implemented!");
    Ok(())
}
