use mlua::{Function, Lua, Result as LuaResult, Table, Value};

// During initialization, PoB calls `SetMainObject` with a callback table.
// Functions defined in this table are used to call back into Lua from Rust.
// Custom callback functions can be added with `SetCallback` and retrieved
// with `GetCallback`.
const CALLBACK_REGISTRY_NAME: &str = "uicallbacks";

pub fn get_callback(lua: &Lua, name: &str) -> LuaResult<Function> {
    let callback_table: Table = lua.named_registry_value(CALLBACK_REGISTRY_NAME)?;
    let callback_function: Value = callback_table.get(name)?;
    if callback_function.is_function() {
        // function defined through `SetCallback`
        return Ok(callback_function.as_function().unwrap().clone());
    } else {
        // look for function in `MainObject`
        let main_object: Table = callback_table.get("MainObject")?;
        let callback_function: Value = main_object.get(name)?;
        if callback_function.is_function() {
            // these functions expect `MainObject` as first argument so we bind it here
            return callback_function.as_function().unwrap().bind(main_object);
        }
    }
    Err(anyhow::anyhow!("Function '{}' not found", name).into())
}

pub fn set_main_object(l: &Lua, main_object: Table) -> LuaResult<()> {
    let callback_table = l.create_table()?;
    callback_table.set("MainObject", main_object)?;
    l.set_named_registry_value(CALLBACK_REGISTRY_NAME, callback_table)?;
    Ok(())
}

pub fn set_custom_callback(l: &Lua, (name, func): (String, Function)) -> LuaResult<()> {
    let callback_table: Table = l.named_registry_value(CALLBACK_REGISTRY_NAME)?;
    callback_table.set(name, func)?;
    Ok(())
}

pub fn get_custom_callback(l: &Lua, name: String) -> LuaResult<Function> {
    let callback_table: Table = l.named_registry_value(CALLBACK_REGISTRY_NAME)?;
    let callback_function: Function = callback_table.get(name)?;
    Ok(callback_function)
}
