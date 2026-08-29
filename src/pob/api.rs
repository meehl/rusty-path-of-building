use crate::pob::{
    api::{
        callback::{call_callback, get_custom_callback, set_custom_callback, set_main_object},
        clipboard::{copy, paste},
        compression::{deflate, inflate},
        console::{console_clear, console_execute, console_print_table, console_printf},
        general::{exit, get_time, open_url, render_init, restart, strip_escapes, take_screenshot},
        image_handle::register_image_handle_api,
        input::{get_cursor_pos, is_key_down, set_cursor_pos, show_cursor},
        lua::{load_module, protected_call, protected_load_module},
        paths::{
            get_runtime_path, get_script_path, get_user_path, get_work_dir, make_dir, remove_dir,
            set_work_dir,
        },
        search_handle::new_search_handle,
        subscript::{abort_subscript, is_subscript_running, launch_subscript},
        window::{
            get_dpi_scale_override, get_screen_scale, get_screen_size, set_dpi_scale_override,
            set_foreground, set_window_title,
        },
    },
    subscript::NativeMultiValue,
};
use mlua::{Lua, MultiValue, Result as LuaResult};

mod callback;
mod clipboard;
mod compression;
mod console;
mod general;
mod image_handle;
mod input;
mod lua;
mod paths;
mod rendering;
mod search_handle;
mod subscript;
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
    globals.set("RemoveDir", lua.create_function(remove_dir)?)?;

    // console
    globals.set("ConPrintf", lua.create_function(console_printf)?)?;
    globals.set("ConExecute", lua.create_function(console_execute)?)?;
    globals.set("ConClear", lua.create_function(console_clear)?)?;
    globals.set("ConPrintTable", lua.create_function(console_print_table)?)?;

    // general
    globals.set("GetTime", lua.create_function(get_time)?)?;
    globals.set("StripEscapes", lua.create_function(strip_escapes)?)?;
    globals.set("Exit", lua.create_function(exit)?)?;
    globals.set("Restart", lua.create_function(restart)?)?;
    globals.set("OpenURL", lua.create_function(open_url)?)?;
    globals.set("RenderInit", lua.create_function(render_init)?)?;
    globals.set("TakeScreenshot", lua.create_function(take_screenshot)?)?;

    // compression
    globals.set("Inflate", lua.create_function(inflate)?)?;
    globals.set("Deflate", lua.create_function(deflate)?)?;

    // search handle
    globals.set("NewFileSearch", lua.create_function(new_search_handle)?)?;

    // image handle
    register_image_handle_api(lua)?;

    // clipboard
    globals.set("Copy", lua.create_function(copy)?)?;
    globals.set("Paste", lua.create_function(paste)?)?;

    // input
    globals.set("GetCursorPos", lua.create_function(get_cursor_pos)?)?;
    globals.set("SetCursorPos", lua.create_function(set_cursor_pos)?)?;
    globals.set("ShowCursor", lua.create_function(show_cursor)?)?;
    globals.set("IsKeyDown", lua.create_function(is_key_down)?)?;

    // window
    globals.set("GetScreenSize", lua.create_function(get_screen_size)?)?;
    globals.set("GetScreenScale", lua.create_function(get_screen_scale)?)?;
    globals.set("SetWindowTitle", lua.create_function(set_window_title)?)?;
    globals.set("SetForeground", lua.create_function(set_foreground)?)?;
    globals.set(
        "SetDPIScaleOverridePercent",
        lua.create_function(set_dpi_scale_override)?,
    )?;
    globals.set(
        "GetDPIScaleOverridePercent",
        lua.create_function(get_dpi_scale_override)?,
    )?;

    // lua
    globals.set("PCall", lua.create_function(protected_call)?)?;
    globals.set("LoadModule", lua.create_function(load_module)?)?;
    globals.set("PLoadModule", lua.create_function(protected_load_module)?)?;

    // subscripts
    globals.set(
        "LaunchSubScript",
        lua.create_function_mut(launch_subscript)?,
    )?;
    globals.set(
        "IsSubScriptRunning",
        lua.create_function(is_subscript_running)?,
    )?;
    globals.set("AbortSubScript", lua.create_function(abort_subscript)?)?;

    // rendering
    rendering::register_globals(lua)?;

    Ok(())
}

// Registers Lua callback functions that can be called from Rust
macro_rules! define_callbacks {
    ($($fn_name:ident => $lua_name:literal ($($arg:ident : $arg_ty:ty),*) -> $ret_ty:ty;)*) => {
        $(
            pub fn $fn_name(lua: &Lua, $($arg: $arg_ty),*) -> LuaResult<$ret_ty> {
                call_callback(lua, $lua_name, ($($arg,)*))
            }
        )*
    };
}

define_callbacks! {
    on_init         => "OnInit"() -> ();
    on_exit         => "OnExit"() -> ();
    on_frame        => "OnFrame"() -> ();
    can_exit        => "CanExit"() -> bool;
    on_key_down     => "OnKeyDown"(key: &str, double_click: bool) -> ();
    on_key_up       => "OnKeyUp"(key: &str) -> ();
    on_char         => "OnChar"(ch: char) -> ();
    on_sub_call     => "OnSubCall"(name: String, args: NativeMultiValue) -> MultiValue;
    on_sub_finished => "OnSubFinished"(id: u64, return_values: NativeMultiValue) -> ();
    on_sub_error    => "OnSubError"(id: u64, error_msg: String) -> ();
}
