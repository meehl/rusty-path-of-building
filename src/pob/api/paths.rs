use mlua::{IntoLuaMulti, Lua, MultiValue, Result as LuaResult, Value};
use std::{fs, path::PathBuf};

use crate::{
    pob::Context,
    util::{change_working_directory, get_executable_dir},
};

pub fn get_user_path(l: &Lua, _: ()) -> LuaResult<PathBuf> {
    let ctx = l.app_data_ref::<Context>().unwrap();
    Ok(ctx.script_dir.join("userdata"))
}

// parent directory of Launch.lua script
pub fn get_script_path(l: &Lua, _: ()) -> LuaResult<PathBuf> {
    let ctx = l.app_data_ref::<Context>().unwrap();
    Ok(ctx.script_dir.clone())
}

// parent directory of executable
pub fn get_runtime_path(_: &Lua, _: ()) -> LuaResult<PathBuf> {
    Ok(get_executable_dir().unwrap_or_default())
}

pub fn get_work_dir(l: &Lua, _: ()) -> LuaResult<PathBuf> {
    let ctx = l.app_data_ref::<Context>().unwrap();
    Ok(ctx.current_working_dir.clone())
}

// NOTE: unused
pub fn set_work_dir(l: &Lua, path: String) -> LuaResult<()> {
    let mut ctx = l.app_data_mut::<Context>().unwrap();
    if change_working_directory(&path).is_ok() {
        ctx.current_working_dir = path.into();
    }
    Ok(())
}

pub fn make_dir(l: &Lua, path: String) -> LuaResult<MultiValue> {
    match fs::create_dir_all(path) {
        // callers expect first return value to be true on success
        Ok(()) => (true).into_lua_multi(l),
        // otherwise it is set to Nil and second return value is set to error msg
        Err(err) => (Value::Nil, err.to_string()).into_lua_multi(l),
    }
}

pub fn remove_dir(l: &Lua, (path, recursive): (String, Option<bool>)) -> LuaResult<MultiValue> {
    let result = if recursive.unwrap_or(false) {
        fs::remove_dir_all(&path)
    } else {
        fs::remove_dir(&path)
    };
    match result {
        Ok(()) => (true).into_lua_multi(l),
        Err(err) => (Value::Nil, err.to_string()).into_lua_multi(l),
    }
}
