use crate::{
    color::Srgba,
    dpi::{LogicalPoint, LogicalRect, NormalizedRect},
};
use ordered_float::OrderedFloat;
use parley::{FontFamily, FontFamilyName};

#[derive(Copy, Clone, Default, Debug, Hash, PartialEq)]
pub enum Alignment {
    #[default]
    Min,
    Center,
    Max,
}

#[derive(Copy, Clone, Default, Debug, Hash, PartialEq)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
}

impl From<FontStyle> for parley::FontStyle {
    fn from(value: FontStyle) -> Self {
        match value {
            FontStyle::Normal => parley::FontStyle::Normal,
            FontStyle::Italic => parley::FontStyle::Italic,
        }
    }
}

#[derive(Clone, Debug, Hash)]
pub struct LayoutSegment<'s> {
    pub text: &'s str,
    pub color: Srgba,
}

#[derive(Clone, Debug)]
pub struct LayoutJob<'s> {
    pub segments: Vec<LayoutSegment<'s>>,
    pub font_family: FontFamily<'static>,
    pub font_size: OrderedFloat<f32>,
    pub line_height: OrderedFloat<f32>,
    pub alignment: Option<Alignment>,
    pub font_weight: Option<OrderedFloat<f32>>,
    pub font_style: FontStyle,
}

impl<'s> LayoutJob<'s> {
    pub fn new(
        font_family: FontFamily<'static>,
        font_size: f32,
        line_height: f32,
        alignment: Option<Alignment>,
        font_weight: Option<f32>,
        font_style: FontStyle,
    ) -> Self {
        Self {
            segments: Vec::new(),
            font_family,
            font_size: font_size.into(),
            line_height: line_height.into(),
            alignment,
            font_weight: font_weight.map(OrderedFloat),
            font_style,
        }
    }

    pub fn append(&mut self, text: &'s str, color: Srgba) {
        self.segments.push(LayoutSegment { text, color });
    }
}

impl std::hash::Hash for LayoutJob<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.segments.hash(state);
        match &self.font_family {
            FontFamily::Single(FontFamilyName::Named(cow)) | FontFamily::Source(cow) => {
                cow.hash(state);
            }
            FontFamily::Single(FontFamilyName::Generic(generic_family)) => {
                generic_family.hash(state);
            }
            FontFamily::List(cow) => {
                for name in cow.iter() {
                    match name {
                        FontFamilyName::Named(cow) => {
                            cow.hash(state);
                        }
                        FontFamilyName::Generic(generic_family) => {
                            generic_family.hash(state);
                        }
                    }
                }
            }
        }
        self.font_size.hash(state);
        self.line_height.hash(state);
        self.alignment.hash(state);
        self.font_weight.hash(state);
        self.font_style.hash(state);
    }
}

pub struct PositionedGlyph {
    // relative to layout origin
    // TODO: introduce new "layout space"?
    pub rect: LogicalRect<f32>,
    pub uv: NormalizedRect,
    pub layer_idx: u32,
    pub color: Srgba,
}

pub struct Layout {
    pub glyphs: Vec<PositionedGlyph>,
    pub width: f32,
    // kept for parley's cursor helper
    pub parley_layout: parley::Layout<Srgba>,
}

impl Layout {
    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn cursor_index_at(&self, point: LogicalPoint<f32>) -> usize {
        parley::Cursor::from_point(&self.parley_layout, point.x, point.y).index()
    }
}
