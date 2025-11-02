use crate::lua::Context;
use mlua::{Lua, Result as LuaResult};

pub fn copy(l: &Lua, text: String) -> LuaResult<()> {
    let ctx = l.app_data_ref::<&'static Context>().unwrap();
    ctx
        .clipboard()
        .set_text(text)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

pub fn paste(l: &Lua, _: ()) -> LuaResult<String> {
    let ctx = l.app_data_ref::<&'static Context>().unwrap();
    let text = ctx
        .clipboard()
        .get_text()
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(text)
}
