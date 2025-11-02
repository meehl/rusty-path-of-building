use crate::{
    api::image_handle::ImageHandle,
    color::Srgba,
    dpi::Uv,
    fonts::{Alignment, FontStyle, LayoutJob},
    lua::Context,
    math::{Point, Quad, Rect, Size},
};
use core::ffi::{c_int, c_void};
use mlua::{
    LightUserData, Lua, Result as LuaResult, UserDataRefMut, Value,
    ffi::{self},
};
use parley::FontFamily;
use regex::Regex;
use std::{borrow::Cow, sync::LazyLock};

pub fn register_globals(lua: &Lua) -> LuaResult<()> {
    let globals = lua.globals();

    // unused functions
    let get_draw_layer = |_: &Lua, ()| -> LuaResult<()> { unimplemented!() };
    let set_blend_mode = |_: &Lua, ()| -> LuaResult<()> { unimplemented!() };
    let get_async_count = |_: &Lua, ()| -> LuaResult<()> { unimplemented!() };
    let set_clear_color = |_: &Lua, ()| -> LuaResult<()> { unimplemented!() };
    globals.set("GetDrawLayer", lua.create_function(get_draw_layer)?)?;
    globals.set("SetBlendMode", lua.create_function(set_blend_mode)?)?;
    globals.set("GetAsyncCount", lua.create_function(get_async_count)?)?;
    globals.set("SetClearColor", lua.create_function(set_clear_color)?)?;

    // rendering functions
    // NOTE: unfortunately, mlua's conversion of function arguments adds a lot of
    // overhead. this is very noticeable for the draw functions which can be called
    // thousands of times per frame. C functions are used to get raw access to
    // the lua stack without the overhead.
    unsafe { globals.set("SetDrawColor", lua.create_c_function(set_draw_color)?)? };
    unsafe { globals.set("SetViewport", lua.create_c_function(set_viewport)?)? };
    unsafe {
        globals.set("SetDrawLayer", lua.create_c_function(set_draw_layer)?)?;
    }
    unsafe { globals.set("DrawImage", lua.create_c_function(draw_image)?)? };
    unsafe { globals.set("DrawImageQuad", lua.create_c_function(draw_image_quad)?)? };
    unsafe {
        globals.set("DrawString", lua.create_c_function(draw_string)?)?;
    }
    unsafe {
        globals.set("DrawStringWidth", lua.create_c_function(get_string_width)?)?;
    }
    globals.set(
        "DrawStringCursorIndex",
        lua.create_function_mut(get_index_at_cur)?,
    )?;

    // NOTE: mlua wraps UserData in a special way to maintain safety guarantees.
    // This wrapper is not exposed by mlua, making it difficult to access the
    // underlying user data from within C functions.
    // This is a helper function that unwraps an ImageHandle and returns a pointer to it
    // See: https://github.com/mlua-rs/mlua/discussions/545#discussioncomment-12530475
    let get_img_handle = lua.create_function(|_, mut ud: UserDataRefMut<ImageHandle>| {
        let vec: *mut ImageHandle = &mut *ud;
        Ok(Value::LightUserData(LightUserData(vec as *mut c_void)))
    })?;
    lua.set_named_registry_value("get_img_handle", get_img_handle)?;
    Ok(())
}

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

macro_rules! img_handle_from_stack {
    ($s:ident, $i:expr) => {
        unsafe {
            match ffi::lua_type($s, $i) {
                ffi::LUA_TNIL => None,
                ffi::LUA_TUSERDATA => {
                    let img_handle = lua_toimghandle($s, $i);
                    if !img_handle.is_null() {
                        if let ImageHandle::Loaded(ref handle) = *img_handle {
                            Some(handle.id())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                t => panic!("Expected Nil or ImageHandle, got {:?}", t),
            }
        }
    };
}

unsafe extern "C" fn lua_toimghandle(state: *mut ffi::lua_State, idx: c_int) -> *mut ImageHandle {
    unsafe {
        let idx = ffi::lua_absindex(state, idx);
        ffi::lua_getfield(state, ffi::LUA_REGISTRYINDEX, c"get_img_handle".as_ptr());
        ffi::lua_pushvalue(state, idx);
        let img_handle = match ffi::lua_pcall(state, 1, 1, 0) {
            ffi::LUA_OK => ffi::lua_touserdata(state, -1) as *mut ImageHandle,
            _ => std::ptr::null_mut(),
        };
        ffi::lua_pop(state, 1);
        img_handle
    }
}

unsafe extern "C-unwind" fn set_draw_color(state: *mut ffi::lua_State) -> c_int {
    //profiling::scope!("set_draw_color");
    let lua_instance = unsafe { Lua::get_or_init_from_ptr(state) };
    let ctx = lua_instance.app_data_ref::<&'static Context>().unwrap();

    let nargs = unsafe { ffi::lua_gettop(state) };
    match nargs {
        // escape_code
        1 => {
            let esc_str = str_from_stack!(state, -nargs);
            let color = Srgba::from_escape_code(esc_str);
            ctx.layers().set_draw_color(color);
        }
        // rgb
        3 => {
            let r = f32_from_stack!(state, -nargs);
            let g = f32_from_stack!(state, -nargs + 1);
            let b = f32_from_stack!(state, -nargs + 2);
            let color = Srgba::new_f32(r, g, b, 1.0);
            ctx.layers().set_draw_color(color);
        }
        // rgba
        4 => {
            let r = f32_from_stack!(state, -nargs);
            let g = f32_from_stack!(state, -nargs + 1);
            let b = f32_from_stack!(state, -nargs + 2);
            let a = f32_from_stack!(state, -nargs + 3);
            let color = Srgba::new_f32(r, g, b, a);
            ctx.layers().set_draw_color(color);
        }
        _ => panic!("Unexpected number of arguments"),
    };

    0
}

unsafe extern "C-unwind" fn set_viewport(state: *mut ffi::lua_State) -> c_int {
    //profiling::scope!("set_viewport");
    let lua_instance = unsafe { Lua::get_or_init_from_ptr(state) };
    let ctx = lua_instance.app_data_ref::<&'static Context>().unwrap();

    let nargs = unsafe { ffi::lua_gettop(state) };
    match nargs {
        0 => ctx
            .layers()
            .set_viewport_from_size(ctx.window().logical_size()),
        4 => {
            let x = f32_from_stack!(state, -nargs);
            let y = f32_from_stack!(state, -nargs + 1);
            let w = f32_from_stack!(state, -nargs + 2);
            let h = f32_from_stack!(state, -nargs + 3);
            let rect = Rect::from_origin_and_size(Point::new(x, y), Size::new(w, h));
            ctx.layers().set_viewport(rect);
        }
        _ => panic!("Unexpected number of arguments"),
    };

    0
}

unsafe extern "C-unwind" fn set_draw_layer(state: *mut ffi::lua_State) -> c_int {
    //profiling::scope!("set_draw_layer");
    let lua_instance = unsafe { Lua::get_or_init_from_ptr(state) };
    let ctx = lua_instance.app_data_ref::<&'static Context>().unwrap();

    let nargs = unsafe { ffi::lua_gettop(state) };

    match nargs {
        1 => {
            let layer = i32_from_stack!(state, -nargs);
            ctx.layers().set_draw_layer(layer, 0);
        }
        2 => {
            let layer = match unsafe { ffi::lua_type(state, -nargs) } {
                ffi::LUA_TNIL => None,
                ffi::LUA_TNUMBER => {
                    let layer = i32_from_stack!(state, -nargs);
                    Some(layer)
                }
                t => panic!("Expected Nil or Number, got {:?}", t),
            };
            let sublayer = i32_from_stack!(state, -nargs + 1);
            if let Some(layer) = layer {
                ctx.layers().set_draw_layer(layer, sublayer);
            } else {
                ctx.layers().set_draw_sublayer(sublayer);
            }
        }
        _ => panic!("Unexpected number of arguments"),
    };

    0
}

unsafe extern "C-unwind" fn draw_image(state: *mut ffi::lua_State) -> c_int {
    //profiling::scope!("draw_image");
    let lua_instance = unsafe { Lua::get_or_init_from_ptr(state) };
    let ctx = lua_instance.app_data_ref::<&'static Context>().unwrap();

    let nargs = unsafe { ffi::lua_gettop(state) };
    if !matches!(nargs, 5 | 6 | 7 | 9 | 10 | 11) {
        panic!("Unexpected number of arguments");
    }

    #[allow(clippy::manual_range_patterns)]
    let parse_uv = matches!(nargs, 9 | 10 | 11);
    let parse_layer_idx = matches!(nargs, 6 | 7 | 10 | 11);

    let texture_id = img_handle_from_stack!(state, -nargs);

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
        Rect::new(Point::new(u1, v1), Point::new(u2, v2))
    } else {
        Rect::default_uv()
    };

    let layer_idx = if parse_layer_idx {
        let layer_idx = i32_from_stack!(state, -nargs + i);
        (layer_idx - 1) as u32
    } else {
        0
    };

    ctx.layers().draw_rect(texture_id, rect, uv, layer_idx);

    0
}

unsafe extern "C-unwind" fn draw_image_quad(state: *mut ffi::lua_State) -> c_int {
    //profiling::scope!("draw_image_quad", format!("args: {:?}", args));
    let lua_instance = unsafe { Lua::get_or_init_from_ptr(state) };
    let ctx = lua_instance.app_data_ref::<&'static Context>().unwrap();

    let nargs = unsafe { ffi::lua_gettop(state) };
    if !matches!(nargs, 9 | 10 | 11 | 17 | 18 | 19) {
        panic!("Unexpected number of arguments");
    }

    #[allow(clippy::manual_range_patterns)]
    let parse_uv = matches!(nargs, 17 | 18 | 19);
    let parse_layer_idx = matches!(nargs, 10 | 11 | 18 | 19);

    let texture_id = img_handle_from_stack!(state, -nargs);

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
        Quad::new(
            Point::new(u1, v1),
            Point::new(u2, v2),
            Point::new(u3, v3),
            Point::new(u4, v4),
        )
    } else {
        Quad::default_uv()
    };

    let layer_idx = if parse_layer_idx {
        let layer_idx = i32_from_stack!(state, -nargs + i);
        (layer_idx - 1) as u32
    } else {
        0
    };

    ctx.layers().draw_quad(texture_id, quad, uv, layer_idx);

    0
}

unsafe extern "C-unwind" fn draw_string(state: *mut ffi::lua_State) -> c_int {
    //profiling::scope!("draw_string");
    let lua_instance = unsafe { Lua::get_or_init_from_ptr(state) };
    let ctx = lua_instance.app_data_ref::<&'static Context>().unwrap();

    let nargs = unsafe { ffi::lua_gettop(state) };

    let x = f32_from_stack!(state, -nargs);
    let y = f32_from_stack!(state, -nargs + 1);
    let alignment = str_from_stack!(state, -nargs + 2);
    let line_height = i32_from_stack!(state, -nargs + 3);
    let font_type = str_from_stack!(state, -nargs + 4);
    let text = str_from_stack!(state, -nargs + 5);

    let alignment = match alignment.parse::<PoBTextAlignment>() {
        Ok(alignment) => alignment,
        Err(_) => panic!("Invalid alignment"),
    };
    let font_type = match font_type.parse::<PoBFontType>() {
        Ok(font_type) => font_type,
        Err(_) => panic!("Invalid font type"),
    };

    let mut position = Point::new(x, y);
    let mut is_absolute_position = false;
    // the position needs to be adjusted for some alignments to match PoBs behavior
    let screen_size = ctx.window().logical_size();
    let halign = match alignment {
        PoBTextAlignment::Left => Alignment::Min,
        PoBTextAlignment::Center => {
            position.x += screen_size.width as f32 / 2.0;
            is_absolute_position = true;
            Alignment::Center
        }
        PoBTextAlignment::Right => {
            position.x = screen_size.width as f32 - position.x;
            is_absolute_position = true;
            Alignment::Max
        }
        PoBTextAlignment::CenterX => Alignment::Center,
        PoBTextAlignment::RightX => Alignment::Max,
    };

    let current_draw_color = ctx.layers().get_draw_color();
    let job = build_layout_job(
        text,
        current_draw_color,
        font_type,
        line_height,
        Some(halign),
    );

    // NOTE: color escape codes modify the current draw color.
    // set current draw color to color of last segment to match PoB's behavior
    if let Some(last_segment) = job.segments.last() {
        ctx.layers().set_draw_color(last_segment.color);
    }

    let layout = ctx.fonts().layout(job, ctx.window().scale_factor);
    ctx
        .layers()
        .draw_text(position, layout, is_absolute_position);

    0
}

unsafe extern "C-unwind" fn get_string_width(state: *mut ffi::lua_State) -> c_int {
    //profiling::scope!("get_string_width");
    let lua_instance = unsafe { Lua::get_or_init_from_ptr(state) };
    let ctx = lua_instance.app_data_ref::<&'static Context>().unwrap();

    let nargs = unsafe { ffi::lua_gettop(state) };

    let line_height = i32_from_stack!(state, -nargs);
    let font_type = str_from_stack!(state, -nargs + 1);
    let text = str_from_stack!(state, -nargs + 2);

    let font_type = match font_type.parse::<PoBFontType>() {
        Ok(font_type) => font_type,
        Err(_) => panic!("Invalid font type"),
    };

    let job = build_layout_job(text, Srgba::WHITE, font_type, line_height, None);
    let width = ctx
        .fonts()
        .get_text_width(job, ctx.window().scale_factor);

    unsafe { ffi::lua_pushnumber(state, width as f64) };
    1
}

fn get_index_at_cur(
    l: &Lua,
    (line_height, font_type, text, cur_x, cur_y): (i32, String, String, f32, f32),
) -> LuaResult<usize> {
    //profiling::scope!("get_char_index_at_cur");
    let ctx = l.app_data_ref::<&'static Context>().unwrap();

    let font_type = font_type.parse::<PoBFontType>()?;

    let job = build_layout_job(&text, Srgba::WHITE, font_type, line_height, None);
    let index = ctx.fonts().get_text_index_at_cursor(
        job,
        Point::new(cur_x, cur_y),
        ctx.window().scale_factor,
    );

    // convert to lua's 1-based indexing
    Ok(index + 1)
}

pub static ESCAPE_STR_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\^(?<idx>[0-9])|\^[x|X](?<hex>[0-9A-Fa-f]{6})").unwrap());

fn build_layout_job<'a>(
    text: &'a str,
    current_color: Srgba,
    font_type: PoBFontType,
    line_height: i32,
    alignment: Option<Alignment>,
) -> LayoutJob<'a> {
    let mut font_weight = None;
    let mut font_style = FontStyle::default();
    let font_family = match font_type {
        PoBFontType::Fixed => FontFamily::Named(Cow::Borrowed("Bitstream Vera Sans Mono")),
        PoBFontType::Var => FontFamily::Named(Cow::Borrowed("Liberation Sans")),
        PoBFontType::VarBold => {
            font_weight = Some(700.0);
            FontFamily::Named(Cow::Borrowed("Liberation Sans"))
        }
        PoBFontType::Fontin => FontFamily::Named(Cow::Borrowed("Fontin")),
        PoBFontType::FontinItalic => FontFamily::Named(Cow::Borrowed("Fontin")),
        PoBFontType::FontinSmallcaps => FontFamily::Named(Cow::Borrowed("Fontin SmallCaps")),
        PoBFontType::FontinSmallcapsItalic => {
            font_style = FontStyle::Italic;
            // use regular Smallcaps with "faux italics"
            FontFamily::Named(Cow::Borrowed("Fontin SmallCaps"))
        }
    };

    // TODO: check how PoB determines font size
    let font_size = (line_height - 2).clamp(11, 32) as f32;

    let mut job = LayoutJob::new(
        font_family,
        font_size,
        line_height as f32,
        alignment,
        font_weight,
        font_style,
    );

    for (color, segment) in PoBString(text).into_iter() {
        job.append(segment, color.unwrap_or(current_color));
    }

    job
}

// PoB strings can contain escape codes that affect the color of subsequent text
pub struct PoBString<'a>(pub &'a str);

impl<'a> PoBString<'a> {
    pub fn strip_escapes(&self) -> String {
        ESCAPE_STR_REGEX.replace_all(self.0, "").to_string()
    }
}

type ColoredSegment<'a> = (Option<Srgba>, &'a str);

impl<'a> IntoIterator for PoBString<'a> {
    type Item = ColoredSegment<'a>;
    type IntoIter = PoBStringSegmentIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        PoBStringSegmentIterator::new(self.0)
    }
}

// Iterates over colored segments
pub struct PoBStringSegmentIterator<'a> {
    haystack: &'a str,
    captures: std::iter::Peekable<regex::CaptureMatches<'static, 'a>>,
    is_first: bool,
    is_done: bool,
}

impl<'a> PoBStringSegmentIterator<'a> {
    fn new(haystack: &'a str) -> Self {
        let captures = ESCAPE_STR_REGEX.captures_iter(haystack).peekable();
        Self {
            haystack,
            captures,
            is_first: true,
            is_done: false,
        }
    }
}

impl<'a> Iterator for PoBStringSegmentIterator<'a> {
    type Item = ColoredSegment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let is_first = core::mem::replace(&mut self.is_first, false);

        match self.captures.peek() {
            Some(capture) => {
                let code_start = capture.get(0).unwrap().start();
                let code_end = capture.get(0).unwrap().end();

                // string didn't start with an escape code.
                // return text up to first code without color.
                if is_first && code_start > 0 {
                    return Some((None, &self.haystack[..code_start]));
                }

                let escape_str = capture.get(0).unwrap().as_str();
                let color = Some(Srgba::from_escape_code(escape_str));

                let _ = self.captures.next(); // pop current code to peek the next one
                match self.captures.peek() {
                    Some(next_code) => {
                        // found another escape code. return text up the next code
                        let next_code_start = next_code.get(0).unwrap().start();
                        Some((color, &self.haystack[code_end..next_code_start]))
                    }
                    None => {
                        // no additional escape codes found. return rest of string
                        self.is_done = true;
                        Some((color, &self.haystack[code_end..]))
                    }
                }
            }
            None => {
                if self.is_done {
                    None
                } else {
                    // string doesn't contain any escape codes.
                    // return entire string without color
                    self.is_done = true;
                    Some((None, self.haystack))
                }
            }
        }
    }
}

// PoB's text alignment is weird
#[derive(Clone, Copy, Debug)]
enum PoBTextAlignment {
    // left-aligned, x coordinate describes top-left corner
    Left,
    // centered in screen space, x coordinate describes offset of text center
    // from screen center. positive values of x move text to right, negative to left.
    Center,
    // right-aligned in screen space, x coordinate describes distance from right edge
    // of text to right edge of screen. positive values move text further to left.
    Right,
    // makes text centered around position, i.e. position.x = horizontal center of text
    CenterX,
    // makes text right-aligned to position, i.e. position.x = right edge of text
    RightX,
}

impl std::str::FromStr for PoBTextAlignment {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "LEFT" => Ok(Self::Left),
            "CENTER" => Ok(Self::Center),
            "RIGHT" => Ok(Self::Right),
            "CENTER_X" => Ok(Self::CenterX),
            "RIGHT_X" => Ok(Self::RightX),
            _ => Err(anyhow::anyhow!(
                "'{}' is not a valid TextFontType variant",
                s
            )),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum PoBFontType {
    Fixed,
    Var,
    VarBold,
    FontinSmallcaps,
    FontinSmallcapsItalic,
    Fontin,
    FontinItalic,
}

impl std::str::FromStr for PoBFontType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "FIXED" => Ok(Self::Fixed),
            "VAR" => Ok(Self::Var),
            "VAR BOLD" => Ok(Self::VarBold),
            "FONTIN SC" => Ok(Self::FontinSmallcaps),
            "FONTIN SC ITALIC" => Ok(Self::FontinSmallcapsItalic),
            "FONTIN" => Ok(Self::Fontin),
            "FONTIN ITALIC" => Ok(Self::FontinItalic),
            _ => Err(anyhow::anyhow!(
                "'{}' is not a valid TextFontType variant",
                s
            )),
        }
    }
}

impl Srgba {
    fn from_escape_code(escape_str: &str) -> Srgba {
        if let Some(caps) = ESCAPE_STR_REGEX.captures(escape_str) {
            if let Some(idx) = caps.name("idx") {
                return match idx.as_str() {
                    "0" => Srgba::from_rgb(0, 0, 0),       //black
                    "1" => Srgba::from_rgb(255, 0, 0),     //red
                    "2" => Srgba::from_rgb(0, 255, 0),     //green
                    "3" => Srgba::from_rgb(0, 0, 255),     //blue
                    "4" => Srgba::from_rgb(255, 255, 0),   //yellow
                    "5" => Srgba::from_rgb(255, 0, 255),   //purple
                    "6" => Srgba::from_rgb(0, 255, 255),   //aqua
                    "7" => Srgba::from_rgb(255, 255, 255), //white
                    "8" => Srgba::from_rgb(178, 178, 178), //gray
                    "9" => Srgba::from_rgb(102, 102, 102), //dark gray
                    _ => unreachable!(),
                };
            }
            if let Some(hex) = caps.name("hex")
                && let Ok(hex_color) = Srgba::from_hex(hex.as_str())
            {
                return hex_color;
            }
        }
        Srgba::WHITE
    }
}
