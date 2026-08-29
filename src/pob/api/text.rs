use crate::{
    color::Srgba,
    dpi::LogicalPoint,
    fonts::{Alignment, FontStyle, LayoutJob, LayoutToLogical},
    math::Point,
    pob::Context,
};
use core::ffi::c_int;
use mlua::{
    Lua, Result as LuaResult,
    ffi::{self},
};
use parley::{FontFamily, FontFamilyName};
use regex::Regex;
use std::{borrow::Cow, sync::LazyLock};

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

pub fn strip_escapes(_: &Lua, text: String) -> LuaResult<String> {
    Ok(PoBString(&text).strip_escapes())
}

pub unsafe extern "C-unwind" fn draw_string(state: *mut ffi::lua_State) -> c_int {
    //profiling::scope!("draw_string");
    let lua_instance = unsafe { Lua::get_or_init_from_ptr(state) };
    let mut ctx = lua_instance.app_data_mut::<Context>().unwrap();

    let nargs = unsafe { ffi::lua_gettop(state) };

    let x = f32_from_stack!(state, -nargs);
    let y = f32_from_stack!(state, -nargs + 1);
    let alignment = str_from_stack!(state, -nargs + 2);
    let line_height = i32_from_stack!(state, -nargs + 3);
    let font_type = str_from_stack!(state, -nargs + 4);
    let text = str_from_stack!(state, -nargs + 5);

    let Ok(alignment) = alignment.parse::<PoBTextAlignment>() else {
        panic!("Invalid alignment");
    };
    let Ok(font_type) = font_type.parse::<PoBFontType>() else {
        panic!("Invalid font type");
    };

    let mut position = LogicalPoint::new(x, y);
    let mut is_absolute_position = false;
    // the position needs to be adjusted for some alignments to match PoBs behavior
    let screen_size = ctx.window_state.logical_size();
    let halign = match alignment {
        PoBTextAlignment::Left => Alignment::Min,
        PoBTextAlignment::Center => {
            position.x += screen_size.width / 2.0;
            is_absolute_position = true;
            Alignment::Center
        }
        PoBTextAlignment::Right => {
            position.x = screen_size.width - position.x;
            is_absolute_position = true;
            Alignment::Max
        }
        PoBTextAlignment::CenterX => Alignment::Center,
        PoBTextAlignment::RightX => Alignment::Max,
    };

    let current_draw_color = ctx.recorder.get_draw_color();
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
        ctx.recorder.set_draw_color(last_segment.color);
    }

    let scale_factor = ctx.window_state.scale_factor();
    let layout = ctx.fonts.borrow_mut().layout(job, scale_factor);

    // apply offset to account for anchor position
    let position = position + layout.anchor_offset(halign);
    // align layout anchor with the physical pixel grid to reduce blurriness
    let position = (position * scale_factor).round() / scale_factor;

    for glyph in &layout.glyphs {
        ctx.recorder.draw_glyph(
            LayoutToLogical::new(position.x, position.y).transform_box(&glyph.rect),
            glyph.uv,
            glyph.color,
            glyph.layer_idx,
            is_absolute_position,
        );
    }

    0
}

pub unsafe extern "C-unwind" fn get_string_width(state: *mut ffi::lua_State) -> c_int {
    //profiling::scope!("get_string_width");
    let lua_instance = unsafe { Lua::get_or_init_from_ptr(state) };
    let ctx = lua_instance.app_data_ref::<Context>().unwrap();

    let nargs = unsafe { ffi::lua_gettop(state) };

    let line_height = i32_from_stack!(state, -nargs);
    let font_type = str_from_stack!(state, -nargs + 1);
    let text = str_from_stack!(state, -nargs + 2);

    let Ok(font_type) = font_type.parse::<PoBFontType>() else {
        panic!("Invalid font type");
    };

    let job = build_layout_job(text, Srgba::WHITE, font_type, line_height, None);
    let layout = ctx
        .fonts
        .borrow_mut()
        .layout(job, ctx.window_state.scale_factor());
    let width = layout.width();

    unsafe { ffi::lua_pushnumber(state, width as f64) };
    1
}

pub fn get_index_at_cur(
    l: &Lua,
    (line_height, font_type, text, cur_x, cur_y): (i32, String, String, f32, f32),
) -> LuaResult<usize> {
    //profiling::scope!("get_char_index_at_cur");
    let ctx = l.app_data_ref::<Context>().unwrap();

    let font_type = font_type.parse::<PoBFontType>()?;

    let job = build_layout_job(&text, Srgba::WHITE, font_type, line_height, None);
    let layout = ctx
        .fonts
        .borrow_mut()
        .layout(job, ctx.window_state.scale_factor());
    let index_stripped = layout.cursor_index_at(Point::new(cur_x, cur_y));

    // build_layout_job() strips all color escape strings from the original string. The
    // resulting [`LayoutJob`] is then passed to get_text_index_at_cursor() which returns an
    // index into the **stripped* string.
    // But PoB expects an index into the **original, unstripped** text. Therefore we need to add
    // the length of all color escapes up until the cursor position to return the right value.
    //
    // TODO: avoid matching and iterating over string twice
    let mut color_escapes_total_length = 0;
    for capture in ESCAPE_STR_REGEX.find_iter(&text) {
        if capture.start() - color_escapes_total_length > index_stripped {
            break;
        }
        color_escapes_total_length += capture.len();
    }

    // add length of color escapes and convert to lua's 1-based indexing
    Ok(index_stripped + color_escapes_total_length + 1)
}

pub static ESCAPE_STR_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\^(?<idx>[0-9])|\^[xX](?<hex>[0-9A-Fa-f]{6})").unwrap());

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
        PoBFontType::Fixed => FontFamily::Single(FontFamilyName::Named(Cow::Borrowed(
            "Bitstream Vera Sans Mono",
        ))),
        PoBFontType::Var => {
            FontFamily::Single(FontFamilyName::Named(Cow::Borrowed("Liberation Sans")))
        }
        PoBFontType::VarBold => {
            font_weight = Some(700.0);
            FontFamily::Single(FontFamilyName::Named(Cow::Borrowed("Liberation Sans")))
        }
        PoBFontType::Fontin => FontFamily::Single(FontFamilyName::Named(Cow::Borrowed("Fontin"))),
        PoBFontType::FontinItalic => {
            FontFamily::Single(FontFamilyName::Named(Cow::Borrowed("Fontin")))
        }
        PoBFontType::FontinSmallcaps => {
            FontFamily::Single(FontFamilyName::Named(Cow::Borrowed("Fontin SmallCaps")))
        }
        PoBFontType::FontinSmallcapsItalic => {
            font_style = FontStyle::Italic;
            // use regular Smallcaps with "faux italics"
            FontFamily::Single(FontFamilyName::Named(Cow::Borrowed("Fontin SmallCaps")))
        }
    };

    // NOTE: This is just an approximation and was chosen based on how it looks.
    //
    // PoB uses pre-rendered font atlases of discrete sizes and selects the appropriate
    // atlas based on the provided height. Rusty-PoB dynamically renders fonts to a
    // cached font atlas to support the selection of arbitrary sizes.
    //
    // TODO: font size in some dropdowns is too small, e.g. socket group selection in
    // 'Calcs' tab
    let font_size = (line_height - 2).max(1) as f32;

    let mut job = LayoutJob::new(
        font_family,
        font_size,
        line_height as f32,
        alignment,
        font_weight,
        font_style,
    );

    for (color, segment) in PoBString(text) {
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
                if let Some(next_code) = self.captures.peek() {
                    // found another escape code. return text up the next code
                    let next_code_start = next_code.get(0).unwrap().start();
                    Some((color, &self.haystack[code_end..next_code_start]))
                } else {
                    // no additional escape codes found. return rest of string
                    self.is_done = true;
                    Some((color, &self.haystack[code_end..]))
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

// PoB's alignment argument is weird!
// It controls:
// - the alignment within the text box
// - the anchor point of the box
// - the relative/absolute positioning of the box
#[derive(Clone, Copy, Debug)]
enum PoBTextAlignment {
    // alignment: left | anchor: top-left corner | position: relative to viewport
    Left,
    // alignment: center | anchor: top-center | position: relative to center of screen
    Center,
    // alignment: right | anchor: top-right | position: relative to right edge of screen
    Right,
    // alignment: center | anchor: top-center | position: relative to viewport
    CenterX,
    // alignment: right | anchor: top-right corner | position: relative to viewport
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
            _ => Err(anyhow::anyhow!("'{s}' is not a valid TextFontType variant")),
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
            _ => Err(anyhow::anyhow!("'{s}' is not a valid TextFontType variant")),
        }
    }
}
