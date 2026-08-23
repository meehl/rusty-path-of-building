use crate::pob::Context;
use mlua::{Lua, Result as LuaResult};

pub fn copy(l: &Lua, text: String) -> LuaResult<()> {
    let mut ctx = l.app_data_mut::<Context>().unwrap();
    ctx.window_state.set_clipboard_text(text);
    Ok(())
}

pub fn paste(l: &Lua, _: ()) -> LuaResult<Option<String>> {
    let mut ctx = l.app_data_mut::<Context>().unwrap();
    Ok(ctx.window_state.get_clipboard_text())
}
