use std::{
    ffi::{CStr, c_int},
    ptr,
};

use mlua::{Lua, Result as LuaResult, ffi};

use crate::{
    dpi::PhysicalSize,
    pob::Context,
    renderer::textures::{TextureHandle, TextureId, TextureOptions},
};

pub struct ImageHandle {
    texture: Option<TextureHandle>,
}

impl ImageHandle {
    #[inline]
    pub const fn new() -> Self {
        Self { texture: None }
    }

    #[inline]
    pub fn texture_id(&self) -> Option<TextureId> {
        self.texture.as_ref().map(TextureHandle::id)
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.texture.is_some()
    }

    #[inline]
    pub fn is_loading(&self) -> bool {
        self.texture.as_ref().is_none_or(TextureHandle::is_loading)
    }

    #[inline]
    pub fn image_size(&self) -> PhysicalSize<u32> {
        self.texture
            .as_ref()
            .and_then(TextureHandle::size)
            .unwrap_or_else(PhysicalSize::zero)
    }
}

// Lua userdata implementation for `ImageHandle`.
//
// `mlua::UserData` instances are wrapped in a special way to maintain Rust
// safety guarantees which introduces additional overhead.
// https://github.com/mlua-rs/mlua/discussions/545#discussioncomment-12518251
//
// To avoid this overhead, `ImageHandle` is stored directly in Lua userdata
// using the raw Lua C API.

const IMAGE_HANDLE_METATABLE: &[u8] = b"runtime.ImageHandle\0";

#[inline]
unsafe fn push_image_handle(state: *mut ffi::lua_State) -> *mut ImageHandle {
    let ptr = unsafe { ffi::lua_newuserdata(state, std::mem::size_of::<ImageHandle>()) }
        .cast::<ImageHandle>();

    // Construct the Rust object directly in Lua-owned memory
    unsafe {
        ptr.write(ImageHandle::new());
    }

    ptr
}

/// Puts `ImageHandle` on the Lua stack
unsafe extern "C-unwind" fn new_image_handle(state: *mut ffi::lua_State) -> c_int {
    unsafe {
        push_image_handle(state);

        // The metatable must already have been registered.
        ffi::luaL_getmetatable(state, IMAGE_HANDLE_METATABLE.as_ptr().cast());
        ffi::lua_setmetatable(state, -2);
    }

    1
}

/// Gets an `ImageHandle` from the Lua stack.
#[inline]
unsafe fn get_image_handle(state: *mut ffi::lua_State, index: c_int) -> *mut ImageHandle {
    unsafe {
        ffi::luaL_checkudata(state, index, IMAGE_HANDLE_METATABLE.as_ptr().cast())
            .cast::<ImageHandle>()
    }
}

#[inline]
pub unsafe fn image_handle_texture_id(
    state: *mut ffi::lua_State,
    index: c_int,
) -> Option<TextureId> {
    if unsafe { ffi::lua_isnil(state, index) } != 0 {
        return None;
    }

    let ptr = unsafe { ffi::lua_touserdata(state, index) }.cast::<ImageHandle>();

    if ptr.is_null() {
        return None;
    }

    unsafe { (*ptr).texture_id() }
}

// Drops the `ImageHandle`
unsafe extern "C-unwind" fn image_handle_gc(state: *mut ffi::lua_State) -> c_int {
    let ptr = unsafe { ffi::lua_touserdata(state, 1) }.cast::<ImageHandle>();

    if !ptr.is_null() {
        unsafe {
            ptr::drop_in_place(ptr);
        }
    }

    0
}

unsafe extern "C-unwind" fn image_handle_is_valid(state: *mut ffi::lua_State) -> c_int {
    let handle = unsafe { get_image_handle(state, 1) };

    let valid = unsafe { (*handle).is_valid() };

    unsafe {
        ffi::lua_pushboolean(state, c_int::from(valid));
    }

    1
}

unsafe extern "C-unwind" fn image_handle_is_loading(state: *mut ffi::lua_State) -> c_int {
    let handle = unsafe { get_image_handle(state, 1) };

    let loading = unsafe { (*handle).is_loading() };

    unsafe {
        ffi::lua_pushboolean(state, c_int::from(loading));
    }

    1
}

unsafe extern "C-unwind" fn image_handle_image_size(state: *mut ffi::lua_State) -> c_int {
    let handle = unsafe { get_image_handle(state, 1) };

    let size = unsafe { (*handle).image_size() };

    unsafe {
        ffi::lua_pushinteger(state, size.width as ffi::lua_Integer);
        ffi::lua_pushinteger(state, size.height as ffi::lua_Integer);
    }

    2
}

unsafe extern "C-unwind" fn image_handle_unload(state: *mut ffi::lua_State) -> c_int {
    let handle = unsafe { get_image_handle(state, 1) };

    unsafe {
        (*handle).texture = None;
    }

    0
}

unsafe extern "C-unwind" fn image_handle_load(state: *mut ffi::lua_State) -> c_int {
    let handle = unsafe { get_image_handle(state, 1) };

    // Path
    let path_ptr = unsafe { ffi::luaL_checklstring(state, 2, ptr::null_mut()) };

    if path_ptr.is_null() {
        unsafe {
            ffi::luaL_error(state, c"Invalid image path".as_ptr());
        }
        unreachable!();
    }

    let path = unsafe { CStr::from_ptr(path_ptr) };

    let path = match path.to_str() {
        Ok(path) => path.to_owned(),
        Err(_) => {
            unsafe {
                ffi::luaL_error(state, c"Image path is not valid UTF-8".as_ptr());
            }
            unreachable!();
        }
    };

    // Flags
    let mut is_async = false;
    let mut generate_mipmaps = false;
    let mut options = TextureOptions::LINEAR_REPEAT;

    let nargs = unsafe { ffi::lua_gettop(state) };

    for index in 3..=nargs {
        if unsafe { ffi::lua_type(state, index) } != ffi::LUA_TSTRING {
            continue;
        }

        let mut len = 0;

        let ptr = unsafe { ffi::lua_tolstring(state, index, &raw mut len) };
        if ptr.is_null() {
            continue;
        }

        let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), len) };

        match bytes {
            b"CLAMP" => {
                options.wrap_mode = wgpu::AddressMode::ClampToEdge;
            }
            b"NEAREST" => {
                options.magnification = wgpu::FilterMode::Nearest;
            }
            b"ASYNC" => {
                is_async = true;
            }
            b"MIPMAP" => {
                generate_mipmaps = true;
            }
            _ => {}
        }
    }

    // Load / update
    let handle = unsafe { &mut *handle };

    let lua = unsafe { Lua::get_or_init_from_ptr(state) };
    let ctx = lua.app_data_ref::<Context>().unwrap();

    match &mut handle.texture {
        Some(texture_handle) => {
            // Keep the existing TextureHandle and update underlying texture
            let _ = ctx.texture_manager.borrow().update_texture(
                texture_handle.id(),
                path,
                options,
                is_async,
                generate_mipmaps,
            );
        }
        None => {
            if let Ok(texture_handle) =
                ctx.texture_manager
                    .borrow()
                    .load_texture(path, options, is_async, generate_mipmaps)
            {
                handle.texture = Some(texture_handle);
            }
        }
    }

    0
}

unsafe fn register_image_handle_metatable(state: *mut ffi::lua_State) {
    unsafe {
        let created = ffi::luaL_newmetatable(state, IMAGE_HANDLE_METATABLE.as_ptr().cast());

        // Create metatable if it doesn't exist already
        if created != 0 {
            // metatable.__gc = image_handle_gc
            ffi::lua_pushcfunction(state, image_handle_gc);
            ffi::lua_setfield(state, -2, c"__gc".as_ptr());

            // Create method table.
            ffi::lua_newtable(state);

            // Load
            ffi::lua_pushcfunction(state, image_handle_load);
            ffi::lua_setfield(state, -2, c"Load".as_ptr());

            // Unload
            ffi::lua_pushcfunction(state, image_handle_unload);
            ffi::lua_setfield(state, -2, c"Unload".as_ptr());

            // IsValid
            ffi::lua_pushcfunction(state, image_handle_is_valid);
            ffi::lua_setfield(state, -2, c"IsValid".as_ptr());

            // IsLoading
            ffi::lua_pushcfunction(state, image_handle_is_loading);
            ffi::lua_setfield(state, -2, c"IsLoading".as_ptr());

            // ImageSize
            ffi::lua_pushcfunction(state, image_handle_image_size);
            ffi::lua_setfield(state, -2, c"ImageSize".as_ptr());

            // metatable.__index = method table
            ffi::lua_setfield(state, -2, c"__index".as_ptr());
        }

        // Pop metatable.
        ffi::lua_pop(state, 1);
    }
}

pub fn register_image_handle_api(lua: &Lua) -> LuaResult<()> {
    let globals = lua.globals();

    unsafe {
        lua.exec_raw::<()>((), |state| {
            register_image_handle_metatable(state);
        })
    }?;

    globals.set("NewImageHandle", unsafe {
        lua.create_c_function(new_image_handle)
    }?)?;

    Ok(())
}
