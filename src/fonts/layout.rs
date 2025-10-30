use crate::{color::Srgba, dpi::LogicalPoint, fonts::rasterizer::RasterizedGlyph};
use ordered_float::OrderedFloat;
use parley::FontFamily;

#[derive(Copy, Clone, Debug, Hash)]
pub enum Alignment {
    Min,
    Center,
    Max,
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
}

impl<'s> LayoutJob<'s> {
    pub fn new(
        font_family: FontFamily<'static>,
        font_size: f32,
        line_height: f32,
        alignment: Option<Alignment>,
        font_weight: Option<f32>,
    ) -> Self {
        Self {
            segments: Vec::new(),
            font_family,
            font_size: font_size.into(),
            line_height: line_height.into(),
            alignment,
            font_weight: font_weight.map(OrderedFloat),
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
            FontFamily::Named(cow) => {
                cow.hash(state);
            }
            FontFamily::Generic(generic_family) => {
                generic_family.hash(state);
            }
        }
        self.font_size.hash(state);
        self.line_height.hash(state);
        self.alignment.hash(state);
        self.font_weight.hash(state);
    }
}

#[derive(Default)]
pub struct LayoutRow {
    pub glyphs: Vec<RasterizedGlyph>,
}

pub struct Layout {
    pub job_hash: u64,
    pub parley_layout: parley::Layout<Srgba>,
    pub rows: Vec<LayoutRow>,
    pub num_of_vertices: usize,
    pub num_of_indices: usize,
}

impl Layout {
    pub fn width(&self) -> f32 {
        self.parley_layout.full_width()
    }

    /// Returns text index at cursor position
    pub fn cursor_index(&self, cursor: LogicalPoint<f32>) -> usize {
        let cursor = parley::Cursor::from_point(&self.parley_layout, cursor.x, cursor.y);
        cursor.index()
    }
}

impl std::hash::Hash for Layout {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.job_hash.hash(state);
    }
}
