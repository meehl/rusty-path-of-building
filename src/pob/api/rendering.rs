use crate::{
    color::Srgba,
    math::{Point, Quad, Rect, Size},
    pob::{Context, api::image_handle::image_handle_texture_id},
};
use core::ffi::c_int;
use mlua::{
    Lua, Result as LuaResult,
    ffi::{self},
};

macro_rules! str_from_stack {
    ($s:ident, $i:expr) => {
        unsafe {
            let mut size = 0;
            let data = ffi::luaL_checklstring($s, $i, &mut size);
            let bytes = std::slice::from_raw_parts(data as *const u8, size);
            std::str::from_utf8_unchecked(bytes)
        }
    };
}

macro_rules! f32_from_stack {
    ($s:ident, $i:expr) => {
        unsafe { ffi::luaL_checknumber($s, $i) } as f32
    };
}

macro_rules! i32_from_stack {
    ($s:ident, $i:expr) => {
        unsafe { ffi::luaL_checkinteger($s, $i) } as i32
    };
}

pub unsafe extern "C-unwind" fn set_draw_color(state: *mut ffi::lua_State) -> c_int {
    //profiling::scope!("set_draw_color");
    let lua_instance = unsafe { Lua::get_or_init_from_ptr(state) };
    let mut ctx = lua_instance.app_data_mut::<Context>().unwrap();

    let nargs = unsafe { ffi::lua_gettop(state) };
    match nargs {
        // escape_code
        1 => {
            let esc_str = str_from_stack!(state, -nargs);
            let color = Srgba::from_escape_code(esc_str);
            ctx.recorder.set_draw_color(color);
        }
        // rgb
        3 => {
            let r = f32_from_stack!(state, -nargs);
            let g = f32_from_stack!(state, -nargs + 1);
            let b = f32_from_stack!(state, -nargs + 2);
            let color = Srgba::new_f32(r, g, b, 1.0);
            ctx.recorder.set_draw_color(color);
        }
        // rgba
        4 => {
            let r = f32_from_stack!(state, -nargs);
            let g = f32_from_stack!(state, -nargs + 1);
            let b = f32_from_stack!(state, -nargs + 2);
            let a = f32_from_stack!(state, -nargs + 3);
            let color = Srgba::new_f32(r, g, b, a);
            ctx.recorder.set_draw_color(color);
        }
        _ => panic!("Unexpected number of arguments"),
    }

    0
}

pub unsafe extern "C-unwind" fn get_draw_color(state: *mut ffi::lua_State) -> c_int {
    //profiling::scope!("get_draw_color");
    let lua_instance = unsafe { Lua::get_or_init_from_ptr(state) };
    let ctx = lua_instance.app_data_ref::<Context>().unwrap();

    let color: [f32; 4] = ctx.recorder.get_draw_color().into();
    unsafe { ffi::lua_pushnumber(state, color[0] as f64) };
    unsafe { ffi::lua_pushnumber(state, color[1] as f64) };
    unsafe { ffi::lua_pushnumber(state, color[2] as f64) };
    unsafe { ffi::lua_pushnumber(state, color[3] as f64) };

    4
}

pub unsafe extern "C-unwind" fn set_viewport(state: *mut ffi::lua_State) -> c_int {
    //profiling::scope!("set_viewport");
    let lua_instance = unsafe { Lua::get_or_init_from_ptr(state) };
    let mut ctx = lua_instance.app_data_mut::<Context>().unwrap();

    let nargs = unsafe { ffi::lua_gettop(state) };
    match nargs {
        0 => ctx.set_recorder_viewport_to_window_size(),
        4 => {
            let x = f32_from_stack!(state, -nargs);
            let y = f32_from_stack!(state, -nargs + 1);
            let w = f32_from_stack!(state, -nargs + 2);
            let h = f32_from_stack!(state, -nargs + 3);
            let rect = Rect::from_origin_and_size(Point::new(x, y), Size::new(w, h));
            ctx.recorder.set_viewport(rect);
        }
        _ => panic!("Unexpected number of arguments"),
    }

    0
}

pub unsafe extern "C-unwind" fn set_draw_layer(state: *mut ffi::lua_State) -> c_int {
    //profiling::scope!("set_draw_layer");
    let lua_instance = unsafe { Lua::get_or_init_from_ptr(state) };
    let mut ctx = lua_instance.app_data_mut::<Context>().unwrap();

    let nargs = unsafe { ffi::lua_gettop(state) };

    match nargs {
        1 => {
            let layer = i32_from_stack!(state, -nargs);
            ctx.recorder.set_draw_layer(layer, 0);
        }
        2 => {
            let layer = match unsafe { ffi::lua_type(state, -nargs) } {
                ffi::LUA_TNIL => None,
                ffi::LUA_TNUMBER => {
                    let layer = i32_from_stack!(state, -nargs);
                    Some(layer)
                }
                t => panic!("Expected Nil or Number, got {t:?}"),
            };
            let sublayer = i32_from_stack!(state, -nargs + 1);
            if let Some(layer) = layer {
                ctx.recorder.set_draw_layer(layer, sublayer);
            } else {
                ctx.recorder.set_draw_sublayer(sublayer);
            }
        }
        _ => panic!("Unexpected number of arguments"),
    }

    0
}

pub unsafe extern "C-unwind" fn draw_image(state: *mut ffi::lua_State) -> c_int {
    //profiling::scope!("draw_image");
    let lua_instance = unsafe { Lua::get_or_init_from_ptr(state) };
    let mut ctx = lua_instance.app_data_mut::<Context>().unwrap();

    let nargs = unsafe { ffi::lua_gettop(state) };
    assert!(
        matches!(nargs, 5 | 6 | 7 | 9 | 10 | 11),
        "Unexpected number of arguments"
    );

    #[allow(clippy::manual_range_patterns)]
    let parse_uv = matches!(nargs, 9 | 10 | 11);
    let parse_layer_idx = matches!(nargs, 6 | 7 | 10 | 11);

    let texture_id = unsafe { image_handle_texture_id(state, -nargs) };

    // left, top, width, height
    let x = f32_from_stack!(state, -nargs + 1);
    let y = f32_from_stack!(state, -nargs + 2);
    let w = f32_from_stack!(state, -nargs + 3);
    let h = f32_from_stack!(state, -nargs + 4);
    let rect = Rect::from_origin_and_size(Point::new(x, y), Size::new(w, h));

    // u1, v1, u2, v2
    let mut i = 5;
    let uv = if parse_uv {
        let u1 = f32_from_stack!(state, -nargs + i);
        let v1 = f32_from_stack!(state, -nargs + i + 1);
        let u2 = f32_from_stack!(state, -nargs + i + 2);
        let v2 = f32_from_stack!(state, -nargs + i + 3);
        i += 4;
        Some(Rect::new(Point::new(u1, v1), Point::new(u2, v2)))
    } else {
        None
    };

    let layer_idx = if parse_layer_idx {
        let layer_idx = i32_from_stack!(state, -nargs + i);
        (layer_idx - 1) as u32
    } else {
        0
    };

    ctx.recorder.draw_image(rect, texture_id, uv, layer_idx);

    0
}

pub unsafe extern "C-unwind" fn draw_image_quad(state: *mut ffi::lua_State) -> c_int {
    //profiling::scope!("draw_image_quad");
    let lua_instance = unsafe { Lua::get_or_init_from_ptr(state) };
    let mut ctx = lua_instance.app_data_mut::<Context>().unwrap();

    let nargs = unsafe { ffi::lua_gettop(state) };
    assert!(
        matches!(nargs, 9 | 10 | 11 | 17 | 18 | 19),
        "Unexpected number of arguments"
    );

    #[allow(clippy::manual_range_patterns)]
    let parse_uv = matches!(nargs, 17 | 18 | 19);
    let parse_layer_idx = matches!(nargs, 10 | 11 | 18 | 19);

    let texture_id = unsafe { image_handle_texture_id(state, -nargs) };

    // x1, y1, x2, y2, ...
    let x1 = f32_from_stack!(state, -nargs + 1);
    let y1 = f32_from_stack!(state, -nargs + 2);
    let x2 = f32_from_stack!(state, -nargs + 3);
    let y2 = f32_from_stack!(state, -nargs + 4);
    let x3 = f32_from_stack!(state, -nargs + 5);
    let y3 = f32_from_stack!(state, -nargs + 6);
    let x4 = f32_from_stack!(state, -nargs + 7);
    let y4 = f32_from_stack!(state, -nargs + 8);
    let quad = Quad::new(
        Point::new(x1, y1),
        Point::new(x2, y2),
        Point::new(x3, y3),
        Point::new(x4, y4),
    );

    // u1, v1, u2, v2, ...
    let mut i = 9;
    let uv = if parse_uv {
        let u1 = f32_from_stack!(state, -nargs + i);
        let v1 = f32_from_stack!(state, -nargs + i + 1);
        let u2 = f32_from_stack!(state, -nargs + i + 2);
        let v2 = f32_from_stack!(state, -nargs + i + 3);
        let u3 = f32_from_stack!(state, -nargs + i + 4);
        let v3 = f32_from_stack!(state, -nargs + i + 5);
        let u4 = f32_from_stack!(state, -nargs + i + 6);
        let v4 = f32_from_stack!(state, -nargs + i + 7);
        i += 8;
        Some(Quad::new(
            Point::new(u1, v1),
            Point::new(u2, v2),
            Point::new(u3, v3),
            Point::new(u4, v4),
        ))
    } else {
        None
    };

    let layer_idx = if parse_layer_idx {
        let layer_idx = i32_from_stack!(state, -nargs + i);
        (layer_idx - 1) as u32
    } else {
        0
    };

    ctx.recorder
        .draw_image_quad(quad, texture_id, uv, layer_idx);

    0
}

pub fn get_draw_layer(l: &Lua, _: ()) -> LuaResult<i32> {
    let ctx = l.app_data_ref::<Context>().unwrap();
    // matching PoB's behavior where only the sublayer is returned
    Ok(ctx.recorder.get_draw_layer().1)
}

pub fn set_blend_mode(_: &Lua, _: ()) -> LuaResult<()> {
    unimplemented!()
}

pub fn get_async_count(_: &Lua, _: ()) -> LuaResult<()> {
    unimplemented!()
}

pub fn set_clear_color(_: &Lua, _: ()) -> LuaResult<()> {
    unimplemented!()
}
