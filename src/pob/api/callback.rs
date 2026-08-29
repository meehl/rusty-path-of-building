//! During initialization, PoB calls `set_main_object` with a callback table. Functions defined in
//! this table are used to call back into Lua from Rust.
//!
//! Custom callback functions can be added with `set_custom_callback` and retrieved with
//! `get_custom_callback`. Functions defined in the main object are shadowed by custom functions
//! with the same name.
use mlua::{FromLuaMulti, Function, IntoLuaMulti, Lua, Result as LuaResult, Table, Value};

const CALLBACK_REGISTRY_NAME: &str = "uicallbacks";

pub fn set_main_object(l: &Lua, main_object: Table) -> LuaResult<()> {
    let callback_table = l.create_table()?;
    callback_table.set("MainObject", main_object)?;
    l.set_named_registry_value(CALLBACK_REGISTRY_NAME, callback_table)
}

pub fn set_custom_callback(l: &Lua, (name, func): (String, Function)) -> LuaResult<()> {
    let callback_table: Table = l.named_registry_value(CALLBACK_REGISTRY_NAME)?;
    callback_table.set(name, func)
}

pub fn get_custom_callback(l: &Lua, name: String) -> LuaResult<Function> {
    let callback_table: Table = l.named_registry_value(CALLBACK_REGISTRY_NAME)?;
    callback_table.get(name)
}

/// Looks up a callback by name.
///
/// Custom callbacks shadow callbacks defined in main object.
fn get_callback(lua: &Lua, name: &str) -> LuaResult<Function> {
    let callback_table: Table = lua.named_registry_value(CALLBACK_REGISTRY_NAME)?;

    // check for custom callbacks first
    if let Value::Function(f) = callback_table.get(name)? {
        return Ok(f);
    }

    // then look look in main object
    if let Value::Table(main_object) = callback_table.get("MainObject")?
        && let Value::Function(f) = main_object.get(name)?
    {
        // these functions expect the main object as first argument so we bind it here
        return f.bind(main_object);
    }

    Err(anyhow::anyhow!("Callback '{name}' not found").into())
}

#[inline]
pub fn call_callback<A, R>(lua: &Lua, name: &str, args: A) -> LuaResult<R>
where
    A: IntoLuaMulti,
    R: FromLuaMulti,
{
    get_callback(lua, name)?.call(args)
}
