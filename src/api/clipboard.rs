use crate::lua::ContextSocket;
use mlua::{Lua, Result as LuaResult};

pub fn copy(l: &Lua, text: String) -> LuaResult<()> {
    let socket = l.app_data_ref::<&'static ContextSocket>().unwrap();
    socket
        .clipboard()
        .set_text(text)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

pub fn paste(l: &Lua, _: ()) -> LuaResult<String> {
    let socket = l.app_data_ref::<&'static ContextSocket>().unwrap();
    let text = socket
        .clipboard()
        .get_text()
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(text)
}
