use mlua::{IntoLuaMulti, Lua, MultiValue, Result as LuaResult, Value};
use std::{fs, path::PathBuf};

use crate::{
    args::Game,
    lua::ContextSocket,
    util::{change_working_directory, get_executable_dir},
};

pub fn get_user_path(_: &Lua, _: ()) -> LuaResult<PathBuf> {
    Ok(Game::user_data_dir())
}

// parent directory of Launch.lua script
pub fn get_script_path(_: &Lua, _: ()) -> LuaResult<PathBuf> {
    Ok(Game::script_dir())
}

// parent directory of executable
pub fn get_runtime_path(_: &Lua, _: ()) -> LuaResult<PathBuf> {
    match get_executable_dir() {
        Ok(exe_path) => Ok(exe_path),
        Err(_) => Ok(PathBuf::new()),
    }
}

pub fn get_work_dir(l: &Lua, _: ()) -> LuaResult<PathBuf> {
    let socket = l.app_data_ref::<&'static ContextSocket>().unwrap();
    Ok(socket.current_working_dir().to_path_buf())
}

// NOTE: unused
pub fn set_work_dir(l: &Lua, path: String) -> LuaResult<()> {
    let socket = l.app_data_ref::<&'static ContextSocket>().unwrap();
    if change_working_directory(&path).is_ok() {
        *socket.current_working_dir() = path.into();
    }
    Ok(())
}

pub fn make_dir(l: &Lua, path: String) -> LuaResult<MultiValue> {
    match fs::create_dir_all(path) {
        // callers expect first return value to be true on success
        Ok(_) => Ok(Value::Boolean(true).into_lua_multi(l)?),
        // otherwise it is set to Nil and second return value is set to error msg
        Err(err) => Ok((Value::Nil, err.to_string()).into_lua_multi(l)?),
    }
}
