use std::{
    fs,
    io::{Read, Write, stdout},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;
use directories::UserDirs;
use flate2::{
    Compression,
    read::{ZlibDecoder, ZlibEncoder},
};
use glob::glob;
use mlua::{
    Function, IntoLua, IntoLuaMulti, Lua, MultiValue, Result as LuaResult, String as LuaString,
    Table, Value, Variadic,
};

use crate::{
    Game,
    api::{image_handle::ImageHandle, rendering::PoBString, search_handle::SearchHandle},
    context::CONTEXT,
    input::{str_as_keycode, str_as_mousebutton},
    util::{change_working_directory, get_executable_dir},
};

mod image_handle;
mod rendering;
mod search_handle;

/// Register functions that can be called from lua
pub fn register_globals(lua: &Lua) -> LuaResult<()> {
    let globals = lua.globals();
    globals.set("SetMainObject", lua.create_function(set_main_object)?)?;
    globals.set("SetCallback", lua.create_function(set_custom_callback)?)?;
    globals.set("GetCallback", lua.create_function(get_custom_callback)?)?;
    globals.set("GetUserPath", lua.create_function(get_user_path)?)?;
    globals.set("GetScriptPath", lua.create_function(get_script_path)?)?;
    globals.set("GetRuntimePath", lua.create_function(get_runtime_path)?)?;
    globals.set("ConPrintf", lua.create_function(console_printf)?)?;
    globals.set("ConExecute", lua.create_function(console_execute)?)?;
    globals.set("ConClear", lua.create_function(console_clear)?)?;
    globals.set("ConPrintTable", lua.create_function(console_print_table)?)?;
    globals.set("PCall", lua.create_function(protected_call)?)?;
    globals.set("GetTime", lua.create_function(get_time)?)?;
    globals.set("MakeDir", lua.create_function(make_dir)?)?;
    globals.set("StripEscapes", lua.create_function(strip_escapes)?)?;
    globals.set("Inflate", lua.create_function(inflate)?)?;
    globals.set("Deflate", lua.create_function(deflate)?)?;
    globals.set("NewFileSearch", lua.create_function(new_search_handle)?)?;
    globals.set("GetWorkDir", lua.create_function(get_work_dir)?)?;
    globals.set("SetWorkDir", lua.create_function(set_work_dir)?)?;
    globals.set("Copy", lua.create_function(copy)?)?;
    globals.set("Paste", lua.create_function(paste)?)?;
    globals.set("GetScreenSize", lua.create_function(get_screen_size)?)?;
    globals.set("GetScreenScale", lua.create_function(get_screen_scale)?)?;
    globals.set("GetCursorPos", lua.create_function(get_cursor_pos)?)?;
    globals.set("IsKeyDown", lua.create_function(is_key_down)?)?;
    globals.set("LoadModule", lua.create_function(load_module)?)?;
    globals.set("PLoadModule", lua.create_function(protected_load_module)?)?;
    globals.set("NewImageHandle", lua.create_function(new_image_handle)?)?;
    globals.set("SetWindowTitle", lua.create_function(set_window_title)?)?;
    globals.set("Restart", lua.create_function(restart)?)?;
    globals.set("OpenURL", lua.create_function(open_url)?)?;
    globals.set("RenderInit", lua.create_function(render_init)?)?;

    // NOTE: not used by PoB
    let get_draw_layer = |_: &Lua, ()| -> LuaResult<()> { unimplemented!() };
    let set_blend_mode = |_: &Lua, ()| -> LuaResult<()> { unimplemented!() };
    let get_async_count = |_: &Lua, ()| -> LuaResult<()> { unimplemented!() };
    let set_cursor_pos = |_: &Lua, ()| -> LuaResult<()> { unimplemented!() };
    let show_cursor = |_: &Lua, ()| -> LuaResult<()> { unimplemented!() };
    let set_clear_color = |_: &Lua, ()| -> LuaResult<()> { unimplemented!() };
    globals.set("GetDrawLayer", lua.create_function(get_draw_layer)?)?;
    globals.set("SetBlendMode", lua.create_function(set_blend_mode)?)?;
    globals.set("GetAsyncCount", lua.create_function(get_async_count)?)?;
    globals.set("SetCursorPos", lua.create_function(set_cursor_pos)?)?;
    globals.set("ShowCursor", lua.create_function(show_cursor)?)?;
    globals.set("SetClearColor", lua.create_function(set_clear_color)?)?;

    rendering::register_globals(lua)?;

    Ok(())
}

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

fn set_main_object(l: &Lua, main_object: Table) -> LuaResult<()> {
    let callback_table = l.create_table()?;
    callback_table.set("MainObject", main_object)?;
    l.set_named_registry_value(CALLBACK_REGISTRY_NAME, callback_table)?;
    Ok(())
}

fn set_custom_callback(l: &Lua, (name, func): (String, Function)) -> LuaResult<()> {
    let callback_table: Table = l.named_registry_value(CALLBACK_REGISTRY_NAME)?;
    callback_table.set(name, func)?;
    Ok(())
}

fn get_custom_callback(l: &Lua, name: String) -> LuaResult<Function> {
    let callback_table: Table = l.named_registry_value(CALLBACK_REGISTRY_NAME)?;
    let callback_function: Function = callback_table.get(name)?;
    Ok(callback_function)
}

fn get_user_path(_: &Lua, _: ()) -> LuaResult<anyhow::Result<PathBuf>> {
    let user_dirs = match UserDirs::new() {
        Some(user_dirs) => user_dirs,
        None => return Ok(Err(anyhow!("Failed to retrieve user's home directory"))),
    };

    match user_dirs.document_dir() {
        Some(docs_dir) => Ok(Ok(docs_dir.canonicalize().unwrap())),
        None => Ok(Err(anyhow!(
            "Failed to retrieve user's documents directory"
        ))),
    }
}

// parent directory of Launch.lua script
fn get_script_path(_: &Lua, _: ()) -> LuaResult<PathBuf> {
    Ok(Game::script_dir())
}

// parent directory of executable
fn get_runtime_path(_: &Lua, _: ()) -> LuaResult<PathBuf> {
    match get_executable_dir() {
        Ok(exe_path) => Ok(exe_path),
        Err(_) => Ok(PathBuf::new()),
    }
}

fn console_printf(l: &Lua, (fmt, args): (String, MultiValue)) -> LuaResult<()> {
    // uses lua's builtin string.format function
    let string_module: Table = l.globals().get("string")?;
    let format_func: Function = string_module.get("format")?;
    let formatted_string = format_func.call::<String>((fmt, args))?;
    println!("{formatted_string}");
    Ok(())
}

fn console_execute(_l: &Lua, _cmd: String) -> LuaResult<()> {
    Ok(())
}

fn console_clear(_l: &Lua, _: ()) -> LuaResult<()> {
    Ok(())
}

fn print_table(table: &Table, indent: usize, recursive: bool) -> LuaResult<()> {
    let mut lock = stdout().lock();
    writeln!(lock, "{{")?;
    for pair in table.pairs::<Value, Value>() {
        let inner_ind = indent + 2;
        let (key, value) = pair?;

        if key.is_string() {
            write!(lock, "{0:>1$}\"{2}\" = ", "", inner_ind, key.to_string()?,)?;
        } else {
            write!(lock, "{0:>1$}{2} = ", "", inner_ind, key.to_string()?,)?;
        }

        if value.is_table() {
            if recursive {
                print_table(value.as_table().unwrap(), indent + 2, recursive)?;
            } else {
                writeln!(lock, "{}", value.to_string()?)?;
            }
        } else if value.is_string() {
            writeln!(lock, "\"{}\"", value.to_string()?)?;
        } else {
            writeln!(lock, "{}", value.to_string()?)?;
        }
    }
    writeln!(lock, "{0:>1$}}}", "", indent)?;
    Ok(())
}

fn console_print_table(_l: &Lua, (table, no_recursive): (Table, Option<bool>)) -> LuaResult<()> {
    print_table(&table, 0, !no_recursive.unwrap_or(true))?;
    Ok(())
}

fn protected_call(l: &Lua, (func, args): (Function, MultiValue)) -> LuaResult<MultiValue> {
    match func.call::<MultiValue>(args) {
        // callers expect first return value to be Nil on success
        Ok(return_values) => Ok(std::iter::once(Value::Nil).chain(return_values).collect()),
        // otherwise it is set to error message.
        Err(err) => Ok(err.to_string().into_lua_multi(l)?),
    }
}

fn get_time(_l: &Lua, _: ()) -> LuaResult<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis())
}

fn make_dir(l: &Lua, path: String) -> LuaResult<MultiValue> {
    match fs::create_dir_all(path) {
        // callers expect first return value to be true on success
        Ok(_) => Ok(Value::Boolean(true).into_lua_multi(l)?),
        // otherwise it is set to Nil and second return value is set to error msg
        Err(err) => Ok((Value::Nil, err.to_string()).into_lua_multi(l)?),
    }
}

fn strip_escapes(_: &Lua, text: String) -> LuaResult<String> {
    Ok(PoBString(&text).strip_escapes())
}

fn inflate(l: &Lua, compressed: LuaString) -> LuaResult<MultiValue> {
    let compressed_bytes = &compressed.as_bytes()[..];

    // prevent decompression of input larger than 128MiB
    if compressed_bytes.len() > (128 << 20) {
        return Ok((Value::Nil, "Input larger than 128 MiB")
            .into_lua_multi(l)
            .unwrap());
    }

    let mut decoder = ZlibDecoder::new(compressed_bytes);
    let mut decompressed = Vec::new();
    match decoder.read_to_end(&mut decompressed) {
        Ok(_) => Ok(l
            .create_string(&decompressed)
            .unwrap()
            .into_lua_multi(l)
            .unwrap()),
        Err(e) => Ok((Value::Nil, e.to_string()).into_lua_multi(l).unwrap()),
    }
}

fn deflate(l: &Lua, uncompressed: LuaString) -> LuaResult<MultiValue> {
    let uncompressed_bytes = &uncompressed.as_bytes()[..];

    // prevent compression of input larger than 128MiB
    if uncompressed_bytes.len() > (128 << 20) {
        return Ok((Value::Nil, "Input larger than 128 MiB")
            .into_lua_multi(l)
            .unwrap());
    }

    let mut encoder = ZlibEncoder::new(uncompressed_bytes, Compression::fast());
    let mut compressed = Vec::new();
    match encoder.read_to_end(&mut compressed) {
        Ok(_) => Ok(l
            .create_string(&compressed)
            .unwrap()
            .into_lua_multi(l)
            .unwrap()),
        Err(e) => Ok((Value::Nil, e.to_string()).into_lua_multi(l).unwrap()),
    }
}

fn new_search_handle(
    l: &Lua,
    (pattern, find_directories): (String, Option<bool>),
) -> LuaResult<Value> {
    if let Ok(paths) = glob(&pattern) {
        let directories_only = find_directories.is_some_and(|x| x);
        let mut handle = SearchHandle::new(paths, directories_only);
        // try to get the first result
        handle.next();
        // only return a handle if at least one file/directory is found
        if handle.current.is_some() {
            return handle.into_lua(l);
        }
    };
    Ok(Value::Nil)
}

fn get_work_dir(_: &Lua, _: ()) -> LuaResult<PathBuf> {
    Ok(CONTEXT.with_borrow(|ctx| ctx.current_working_dir().to_path_buf()))
}

// NOTE: unused
fn set_work_dir(_: &Lua, path: String) -> LuaResult<()> {
    CONTEXT.with_borrow_mut(|ctx| ctx.set_current_working_dir(path.into()));
    Ok(())
}

fn copy(_: &Lua, text: String) -> LuaResult<()> {
    CONTEXT.with_borrow_mut(|ctx| {
        ctx.clipboard
            .set_text(text)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;
    Ok(())
}

fn paste(_: &Lua, _: ()) -> LuaResult<String> {
    let text = CONTEXT.with_borrow_mut(|ctx| {
        ctx.clipboard
            .get_text()
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;
    Ok(text)
}

// NOTE: needs to return physical size if this MR gets merged:
// https://github.com/PathOfBuildingCommunity/PathOfBuilding-PoE2/pull/1420
fn get_screen_size(_: &Lua, _: ()) -> LuaResult<(u32, u32)> {
    let size = CONTEXT.with_borrow(|ctx| ctx.screen_size_logical());
    Ok((size.width, size.height))
}

fn get_screen_scale(_: &Lua, _: ()) -> LuaResult<f32> {
    Ok(CONTEXT.with_borrow(|ctx| ctx.pixels_per_point() as f32))
}

fn get_cursor_pos(_: &Lua, _: ()) -> LuaResult<(u32, u32)> {
    let pos = CONTEXT.with_borrow(|ctx| ctx.mouse_pos_logical());
    Ok((pos.x as u32, pos.y as u32))
}

fn is_key_down(_: &Lua, key_name: String) -> LuaResult<bool> {
    if let Some(code) = str_as_keycode(&key_name) {
        Ok(CONTEXT.with_borrow(|ctx| ctx.input_state.key_pressed(code)))
    } else if let Some(button) = str_as_mousebutton(&key_name) {
        Ok(CONTEXT.with_borrow(|ctx| ctx.input_state.mouse_pressed(button)))
    } else {
        Ok(false)
    }
}

fn load_module(l: &Lua, (name, args): (String, MultiValue)) -> LuaResult<MultiValue> {
    let mut module_path = Game::script_dir().join(name);
    if module_path.extension().is_none() {
        module_path.set_extension("lua");
    }

    change_working_directory(Game::script_dir().as_path())?;
    let result = l.load(module_path).call::<MultiValue>(args);
    CONTEXT.with_borrow(|ctx| change_working_directory(ctx.current_working_dir()))?;
    result
}

fn protected_load_module(l: &Lua, (name, args): (String, MultiValue)) -> LuaResult<MultiValue> {
    let mut module_path = Game::script_dir().join(name);
    if module_path.extension().is_none() {
        module_path.set_extension("lua");
    }

    change_working_directory(Game::script_dir().as_path())?;
    let result = match l.load(module_path).call::<MultiValue>(args) {
        // on success, callers expect a Nil followed by return values
        Ok(res) => Ok(std::iter::once(Value::Nil).chain(res).collect()),
        // otherwise it is set to error message.
        Err(err) => Ok(err.to_string().into_lua_multi(l)?),
    };
    CONTEXT.with_borrow(|ctx| change_working_directory(ctx.current_working_dir()))?;
    result
}

fn new_image_handle(_: &Lua, _: ()) -> LuaResult<ImageHandle> {
    Ok(ImageHandle::Unloaded)
}

fn set_window_title(_: &Lua, title: String) -> LuaResult<()> {
    CONTEXT.with_borrow(|ctx| ctx.set_window_title(&title));
    Ok(())
}

fn restart(_: &Lua, _: ()) -> LuaResult<()> {
    CONTEXT.with_borrow_mut(|ctx| ctx.needs_restart = true);
    Ok(())
}

fn open_url(l: &Lua, url: String) -> LuaResult<MultiValue> {
    match open::that(url) {
        Ok(_) => Ok(().into_lua_multi(l)?),
        Err(_) => Ok("Unable to open url!".into_lua_multi(l)?),
    }
}

fn render_init(_: &Lua, _features: Variadic<String>) -> LuaResult<()> {
    Ok(())
}
