use crate::lua::Context;
use mlua::{Lua, Result as LuaResult};

pub fn copy(l: &Lua, text: String) -> LuaResult<()> {
    let ctx = l.app_data_ref::<&'static Context>().unwrap();
    ctx.window().set_clipboard_text(text);
    Ok(())
}

pub fn paste(l: &Lua, _: ()) -> LuaResult<Option<String>> {
    let ctx = l.app_data_ref::<&'static Context>().unwrap();
    Ok(ctx.window().get_clipboard_text())
}
