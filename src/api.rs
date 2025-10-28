pub use crate::api::callback::get_callback;
use crate::{
    api::{
        callback::{get_custom_callback, set_custom_callback, set_main_object},
        clipboard::{copy, paste},
        compression::{deflate, inflate},
        console::{console_clear, console_execute, console_print_table, console_printf},
        image_handle::new_image_handle,
        input::{get_cursor_pos, is_key_down},
        lua::{load_module, protected_call, protected_load_module},
        paths::{
            get_runtime_path, get_script_path, get_user_path, get_work_dir, make_dir, set_work_dir,
        },
        rendering::PoBString,
        search_handle::new_search_handle,
        window::{get_screen_scale, get_screen_size, set_foreground, set_window_title},
    },
    lua::ContextSocket,
};
use mlua::{IntoLuaMulti, Lua, MultiValue, Result as LuaResult, Variadic};
use std::time::{SystemTime, UNIX_EPOCH};

mod callback;
mod clipboard;
mod compression;
mod console;
mod image_handle;
mod input;
mod lua;
mod paths;
mod rendering;
mod search_handle;
mod window;

/// Register functions that can be called from lua
pub fn register_globals(lua: &Lua) -> LuaResult<()> {
    let globals = lua.globals();

    // callbacks
    globals.set("SetMainObject", lua.create_function(set_main_object)?)?;
    globals.set("SetCallback", lua.create_function(set_custom_callback)?)?;
    globals.set("GetCallback", lua.create_function(get_custom_callback)?)?;

    // paths
    globals.set("GetUserPath", lua.create_function(get_user_path)?)?;
    globals.set("GetScriptPath", lua.create_function(get_script_path)?)?;
    globals.set("GetRuntimePath", lua.create_function(get_runtime_path)?)?;
    globals.set("GetWorkDir", lua.create_function(get_work_dir)?)?;
    globals.set("SetWorkDir", lua.create_function(set_work_dir)?)?;
    globals.set("MakeDir", lua.create_function(make_dir)?)?;

    // console
    globals.set("ConPrintf", lua.create_function(console_printf)?)?;
    globals.set("ConExecute", lua.create_function(console_execute)?)?;
    globals.set("ConClear", lua.create_function(console_clear)?)?;
    globals.set("ConPrintTable", lua.create_function(console_print_table)?)?;

    // general
    globals.set("GetTime", lua.create_function(get_time)?)?;
    globals.set("StripEscapes", lua.create_function(strip_escapes)?)?;
    globals.set("Restart", lua.create_function(restart)?)?;
    globals.set("OpenURL", lua.create_function(open_url)?)?;
    globals.set("RenderInit", lua.create_function(render_init)?)?;

    // compression
    globals.set("Inflate", lua.create_function(inflate)?)?;
    globals.set("Deflate", lua.create_function(deflate)?)?;

    // search handle
    globals.set("NewFileSearch", lua.create_function(new_search_handle)?)?;

    // image handle
    globals.set("NewImageHandle", lua.create_function(new_image_handle)?)?;

    // clipboard
    globals.set("Copy", lua.create_function(copy)?)?;
    globals.set("Paste", lua.create_function(paste)?)?;

    // input
    globals.set("GetCursorPos", lua.create_function(get_cursor_pos)?)?;
    globals.set("IsKeyDown", lua.create_function(is_key_down)?)?;

    // window
    globals.set("GetScreenSize", lua.create_function(get_screen_size)?)?;
    globals.set("GetScreenScale", lua.create_function(get_screen_scale)?)?;
    globals.set("SetWindowTitle", lua.create_function(set_window_title)?)?;
    globals.set("SetForeground", lua.create_function(set_foreground)?)?;

    // lua
    globals.set("PCall", lua.create_function(protected_call)?)?;
    globals.set("LoadModule", lua.create_function(load_module)?)?;
    globals.set("PLoadModule", lua.create_function(protected_load_module)?)?;

    // NOTE: not used by PoB
    let set_cursor_pos = |_: &Lua, ()| -> LuaResult<()> { unimplemented!() };
    let show_cursor = |_: &Lua, ()| -> LuaResult<()> { unimplemented!() };
    globals.set("SetCursorPos", lua.create_function(set_cursor_pos)?)?;
    globals.set("ShowCursor", lua.create_function(show_cursor)?)?;

    // rendering
    rendering::register_globals(lua)?;

    Ok(())
}

fn get_time(_l: &Lua, _: ()) -> LuaResult<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis())
}

fn strip_escapes(_: &Lua, text: String) -> LuaResult<String> {
    Ok(PoBString(&text).strip_escapes())
}

fn restart(l: &Lua, _: ()) -> LuaResult<()> {
    let socket = l.app_data_ref::<&'static ContextSocket>().unwrap();
    *socket.needs_restart() = true;
    Ok(())
}

fn open_url(l: &Lua, url: String) -> LuaResult<MultiValue> {
    match open::that(url) {
        Ok(_) => Ok(().into_lua_multi(l)?),
        Err(_) => Ok("Unable to open url!".into_lua_multi(l)?),
    }
}

fn render_init(l: &Lua, features: Variadic<String>) -> LuaResult<()> {
    let socket = l.app_data_ref::<&'static ContextSocket>().unwrap();
    for feature in features {
        if feature == "DPI_AWARE" {
            *socket.is_dpi_aware() = true;
        }
    }
    Ok(())
}
