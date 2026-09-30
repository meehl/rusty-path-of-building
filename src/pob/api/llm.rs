use mlua::{Lua, Result as LuaResult, Table};

use crate::pob::Context;

/// `LlmChat(bodyJson)` starts a chat completion request and returns its id.
///
/// The result is delivered later through the `OnLlmResult(id, body, errMsg)` callback.
pub fn llm_chat(l: &Lua, body: String) -> LuaResult<u64> {
    let ctx = l.app_data_ref::<Context>().unwrap();
    let id = ctx.llm_manager.borrow_mut().send(body);
    Ok(id)
}

/// `LlmCancel(id)` discards a request that is no longer relevant to the current tab/build.
pub fn llm_cancel(l: &Lua, id: u64) -> LuaResult<()> {
    let ctx = l.app_data_ref::<Context>().unwrap();
    ctx.llm_manager.borrow_mut().cancel(id);
    Ok(())
}

/// `LlmGetConfig()` returns `{ hasKey = bool, models = { ... } }`.
///
/// The API key itself is never exposed to Lua.
pub fn llm_get_config(l: &Lua, _: ()) -> LuaResult<Table> {
    let ctx = l.app_data_ref::<Context>().unwrap();
    let llm_manager = ctx.llm_manager.borrow();
    let config = llm_manager.config();

    let table = l.create_table()?;
    table.set("hasKey", config.api_key().is_some())?;
    table.set("baseUrl", config.base_url())?;
    table.set(
        "models",
        l.create_sequence_from(config.models().iter().map(String::as_str))?,
    )?;
    Ok(table)
}
