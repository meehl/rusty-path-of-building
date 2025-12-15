use crate::{lua::Context, util::change_working_directory};
use mlua::{Function, IntoLuaMulti, Lua, MultiValue, Result as LuaResult, Value};
use std::env;

pub fn protected_call(l: &Lua, (func, args): (Function, MultiValue)) -> LuaResult<MultiValue> {
    match func.call::<MultiValue>(args) {
        // callers expect first return value to be Nil on success
        Ok(return_values) => Ok(std::iter::once(Value::Nil).chain(return_values).collect()),
        // otherwise it is set to error message.
        Err(err) => Ok(err.to_string().into_lua_multi(l)?),
    }
}

pub fn load_module(l: &Lua, (name, args): (String, MultiValue)) -> LuaResult<MultiValue> {
    let ctx = l.app_data_ref::<&'static Context>().unwrap();
    let mut module_path = ctx.script_dir().join(name);
    if module_path.extension().is_none() {
        module_path.set_extension("lua");
    }

    let current_dir = env::current_dir()?;
    change_working_directory(ctx.script_dir().as_path())?;
    let result = l.load(module_path).call::<MultiValue>(args);
    change_working_directory(current_dir)?;
    result
}

pub fn protected_load_module(l: &Lua, (name, args): (String, MultiValue)) -> LuaResult<MultiValue> {
    let ctx = l.app_data_ref::<&'static Context>().unwrap();
    let mut module_path = ctx.script_dir().join(name);
    if module_path.extension().is_none() {
        module_path.set_extension("lua");
    }

    let current_dir = env::current_dir()?;
    change_working_directory(ctx.script_dir().as_path())?;
    let result = match l.load(module_path).call::<MultiValue>(args) {
        // on success, callers expect a Nil followed by return values
        Ok(res) => Ok(std::iter::once(Value::Nil).chain(res).collect()),
        // otherwise it is set to error message.
        Err(err) => Ok(err.to_string().into_lua_multi(l)?),
    };
    change_working_directory(current_dir)?;
    result
}
