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
            let bytes = std::slice::from_raw_parts(data.cast::<u8>(), size);
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

    unsafe { ffi::lua_pushnumber(state, f64::from(width)) };
    1
}

pub fn get_index_at_cur(
    l: &Lua,
    (line_height, font_type, text, cur_x, cur_y): (i32, String, String, f32, f32),
) -> LuaResult<usize> {
    //profiling::scope!("get_char_index_at_cur");
    let ctx = l.app_data_ref::<Context>().unwrap();

    let font_type = font_type.parse::<PoBFontType>()?;

    let (job, offset_map) =
        build_layout_job_with_offsets(&text, Srgba::WHITE, font_type, line_height, None);
    let layout = ctx
        .fonts
        .borrow_mut()
        .layout(job, ctx.window_state.scale_factor());

    // `cursor_index_at` returns an offset into the **stripped** text, but PoB expects on offset
    // into the original, **unstripped** text. A `StrippedToOriginalOffsetMap` is used to do the
    // conversion.
    let stripped_index = layout.cursor_index_at(Point::new(cur_x, cur_y));
    let original_index = offset_map.to_original(stripped_index);

    // convert to lua's 1-based indexing
    Ok(original_index + 1)
}

pub static ESCAPE_STR_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\^(?<idx>[0-9])|\^[xX](?<hex>[0-9A-Fa-f]{6})").unwrap());

fn new_layout_job<'a>(
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

    LayoutJob::new(
        font_family,
        font_size,
        line_height as f32,
        alignment,
        font_weight,
        font_style,
    )
}

fn build_layout_job<'a>(
    text: &'a str,
    current_color: Srgba,
    font_type: PoBFontType,
    line_height: i32,
    alignment: Option<Alignment>,
) -> LayoutJob<'a> {
    let mut job = new_layout_job(font_type, line_height, alignment);

    for ColoredSegment {
        color,
        text,
        original_start: _,
    } in PoBString(text)
    {
        job.append(text, color.unwrap_or(current_color));
    }

    job
}

/// Maps byte offset in stripped text back to offset in original text.
#[derive(Debug, Default, Clone)]
struct StrippedToOriginalOffsetMap {
    segments: Vec<(usize, usize)>,
}

impl StrippedToOriginalOffsetMap {
    fn push_segment(&mut self, stripped_start: usize, original_start: usize) {
        self.segments.push((stripped_start, original_start));
    }

    fn to_original(&self, stripped_index: usize) -> usize {
        assert!(!self.segments.is_empty());
        let i = self
            .segments
            .partition_point(|&(stripped_start, _)| stripped_start <= stripped_index);
        let (stripped_start, original_start) = self.segments[i - 1];
        original_start + (stripped_index - stripped_start)
    }
}

/// Returns a `StrippedToOriginalOffsetMap` alongside the `LayoutJob`.
fn build_layout_job_with_offsets<'a>(
    text: &'a str,
    current_color: Srgba,
    font_type: PoBFontType,
    line_height: i32,
    alignment: Option<Alignment>,
) -> (LayoutJob<'a>, StrippedToOriginalOffsetMap) {
    let mut job = new_layout_job(font_type, line_height, alignment);
    let mut offset_map = StrippedToOriginalOffsetMap::default();
    let mut stripped_offset = 0;

    for ColoredSegment {
        color,
        text,
        original_start,
    } in PoBString(text)
    {
        offset_map.push_segment(stripped_offset, original_start);
        job.append(text, color.unwrap_or(current_color));
        stripped_offset += text.len();
    }

    (job, offset_map)
}

/// A string constructed by Lua which may contain color escapes.
pub struct PoBString<'a>(pub &'a str);

impl<'a> PoBString<'a> {
    /// Returns a copy of the string with all color escapes removed.
    pub fn strip_escapes(&self) -> String {
        ESCAPE_STR_REGEX.replace_all(self.0, "").to_string()
    }
}

impl<'a> IntoIterator for PoBString<'a> {
    type Item = ColoredSegment<'a>;
    type IntoIter = PoBStringSegmentIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        PoBStringSegmentIterator::new(self.0)
    }
}

/// A segment of text that shares the same color.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ColoredSegment<'a> {
    pub color: Option<Srgba>,
    pub text: &'a str,
    /// The byte offset of `text` within the **original**, unstripped string.
    pub original_start: usize,
}

/// Iterates over the [`ColoredSegment`]s of a [`PoBString`].
pub struct PoBStringSegmentIterator<'a> {
    haystack: &'a str,
    captures: regex::CaptureMatches<'static, 'a>,
    /// Byte offset in `haystack` where the next segment's text begins.
    cursor: usize,
    /// Color of the next segment.
    color: Option<Srgba>,
    done: bool,
}

impl<'a> PoBStringSegmentIterator<'a> {
    fn new(haystack: &'a str) -> Self {
        Self {
            haystack,
            captures: ESCAPE_STR_REGEX.captures_iter(haystack),
            cursor: 0,
            color: None,
            done: false,
        }
    }
}

impl<'a> Iterator for PoBStringSegmentIterator<'a> {
    type Item = ColoredSegment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        loop {
            if let Some(captures) = self.captures.next() {
                let m = &captures.get(0).expect("group 0 always matches");
                let start = self.cursor;
                let text = &self.haystack[start..m.start()];
                let color = self.color;

                self.cursor = m.end();
                self.color = Some(Srgba::from_escape_code(m.as_str()));

                // check for case where string starts with escape code
                if text.is_empty() && color.is_none() {
                    continue;
                }

                return Some(ColoredSegment {
                    color,
                    text,
                    original_start: start,
                });
            }

            self.done = true;
            return Some(ColoredSegment {
                color: self.color,
                text: &self.haystack[self.cursor..],
                original_start: self.cursor,
            });
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
            _ => Err(anyhow::anyhow!(
                "'{s}' is not a valid PoBTextAlignment variant"
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
            _ => Err(anyhow::anyhow!("'{s}' is not a valid PoBFontType variant")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // helper for collecting segments
    fn segs(text: &str) -> Vec<(Option<Srgba>, &str, usize)> {
        PoBString(text)
            .into_iter()
            .map(|s| (s.color, s.text, s.original_start))
            .collect()
    }

    #[test]
    fn single_segment() {
        assert_eq!(segs("Hello, world!"), vec![(None, "Hello, world!", 0)]);
    }

    #[test]
    fn empty_string() {
        assert_eq!(segs(""), vec![(None, "", 0)]);
    }

    #[test]
    fn escape_at_start() {
        assert_eq!(
            segs("^3Hello, world!"),
            vec![(Some(Srgba::from_rgb(0, 0, 255)), "Hello, world!", 2)]
        );
    }

    #[test]
    fn no_escape_at_start() {
        assert_eq!(
            segs("Hello, ^1world!"),
            vec![
                (None, "Hello, ", 0),
                (Some(Srgba::from_rgb(255, 0, 0)), "world!", 9)
            ]
        );
    }

    #[test]
    fn adjacent_escapes() {
        assert_eq!(
            segs("Hello, ^1^2world!"),
            vec![
                (None, "Hello, ", 0),
                (Some(Srgba::from_rgb(255, 0, 0)), "", 9),
                (Some(Srgba::from_rgb(0, 255, 0)), "world!", 11)
            ]
        );
    }

    #[test]
    fn trailing_escape() {
        assert_eq!(
            segs("Hello, world!^3"),
            vec![
                (None, "Hello, world!", 0),
                (Some(Srgba::from_rgb(0, 0, 255)), "", 15),
            ]
        );
    }

    #[test]
    fn hex_escape() {
        assert_eq!(
            segs("^xFF0000Hello, ^X00ff00world!"),
            vec![
                (Some(Srgba::from_rgb(255, 0, 0)), "Hello, ", 8),
                (Some(Srgba::from_rgb(0, 255, 0)), "world!", 23),
            ]
        );
    }

    #[test]
    fn cursor_offset_empty() {
        let mut offset_map = StrippedToOriginalOffsetMap::default();
        for seg in PoBString("") {
            offset_map.push_segment(0, seg.original_start);
        }
        assert_eq!(offset_map.to_original(0), 0);
    }

    #[test]
    fn cursor_offset_no_escape() {
        let mut offset_map = StrippedToOriginalOffsetMap::default();
        let mut stripped_offset = 0;
        for seg in PoBString("Hello, World!") {
            offset_map.push_segment(stripped_offset, seg.original_start);
            stripped_offset += seg.text.len();
        }
        assert_eq!(offset_map.to_original(0), 0);
        assert_eq!(offset_map.to_original(3), 3);
        assert_eq!(offset_map.to_original(6), 6);
    }

    #[test]
    fn cursor_offset() {
        let mut offset_map = StrippedToOriginalOffsetMap::default();
        let mut stripped_offset = 0;
        for seg in PoBString("^1Hello^xFF0000World^3!") {
            offset_map.push_segment(stripped_offset, seg.original_start);
            stripped_offset += seg.text.len();
        }
        assert_eq!(offset_map.to_original(0), 2);
        assert_eq!(offset_map.to_original(3), 5);
        assert_eq!(offset_map.to_original(6), 16);
        assert_eq!(offset_map.to_original(11), 23);
    }
}
