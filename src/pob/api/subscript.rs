use mlua::{Lua, MultiValue, Result as LuaResult};

use crate::pob::Context;

pub fn launch_subscript(
    l: &Lua,
    (script_text, func_list, sub_list, args): (String, String, String, MultiValue),
) -> LuaResult<u64> {
    let blocking_calls = func_list
        .split(',')
        .map(str::trim)
        .filter(|&s| !s.is_empty())
        .map(String::from)
        .collect();
    let nonblocking_calls = sub_list
        .split(',')
        .map(str::trim)
        .filter(|&s| !s.is_empty())
        .map(String::from)
        .collect();
    let arguments = args.try_into()?;

    let ctx = l.app_data_ref::<Context>().unwrap();
    let script_dir = ctx.script_dir.clone();
    let subscript_id = ctx.subscript_manager.borrow_mut().push(
        script_dir,
        script_text,
        blocking_calls,
        nonblocking_calls,
        arguments,
    );

    Ok(subscript_id)
}

pub fn is_subscript_running(l: &Lua, subscript_id: u64) -> LuaResult<bool> {
    let ctx = l.app_data_ref::<Context>().unwrap();
    Ok(ctx.subscript_manager.borrow().is_running(subscript_id))
}

pub fn abort_subscript(_: &Lua, _subscript_id: u64) -> LuaResult<()> {
    unimplemented!()
}
