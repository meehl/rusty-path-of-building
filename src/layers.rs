use std::collections::BTreeMap;

use crate::{
    color::Srgba,
    dpi::{LogicalPoint, LogicalQuad, LogicalRect, LogicalSize, NormalizedQuad, NormalizedRect},
    fonts::Layout,
    renderer::{
        primitives::{
            ClippedPrimitive, DrawPrimitive, QuadPrimitive, QuadTexture, RectPrimitive,
            RectTexture, TextPrimitive,
        },
        textures::TextureId,
    },
    util::calculate_hash,
};

/// Holds the draw primitives for each layer.
///
/// Adding a primitive places it in currently set layer. Positions are interpreted as being relative to
/// the current viewport. They are translated into absolute positions (screen positions) and
/// clipped by the viewport.
#[derive(Default)]
pub struct Layers {
    layers: BTreeMap<(i32, i32), Vec<ClippedPrimitive>>,
    current_layer: (i32, i32),
    viewport: LogicalRect<f32>,
    current_draw_color: Srgba,
}

impl Layers {
    pub fn reset(&mut self) {
        self.current_layer = (0, 0);
        self.layers.clear();
        self.current_draw_color = Srgba::TRANSPARENT;
    }

    /// Consume primitives and return an iterator over them in drawing order.
    pub fn consume_layers(&mut self) -> Box<dyn Iterator<Item = ClippedPrimitive>> {
        let layers = std::mem::take(&mut self.layers);
        Box::new(layers.into_values().flatten())
    }

    pub fn set_viewport(&mut self, viewport: LogicalRect<f32>) {
        self.viewport = viewport;
    }

    pub fn set_viewport_from_size(&mut self, size: LogicalSize<u32>) {
        self.set_viewport(LogicalRect::from_size(size).cast());
    }

    pub fn set_draw_layer(&mut self, layer: i32, sublayer: i32) {
        self.current_layer = (layer, sublayer);
    }

    pub fn set_draw_sublayer(&mut self, sublayer: i32) {
        self.set_draw_layer(self.current_layer.0, sublayer);
    }

    pub fn set_draw_color(&mut self, color: Srgba) {
        self.current_draw_color = color;
    }

    pub fn get_draw_color(&self) -> Srgba {
        self.current_draw_color
    }

    pub fn draw_rect(
        &mut self,
        texture_id: Option<TextureId>,
        rect: LogicalRect<f32>,
        uv: NormalizedRect,
        layer_idx: u32,
    ) {
        let texture = texture_id.map(|id| RectTexture::new(id, uv, layer_idx));
        let primitive = RectPrimitive::new(rect, self.current_draw_color, texture);
        self.add_rect(primitive);
    }

    pub fn draw_quad(
        &mut self,
        texture_id: Option<TextureId>,
        quad: LogicalQuad<f32>,
        uv: NormalizedQuad,
        layer_idx: u32,
    ) {
        let texture = texture_id.map(|id| QuadTexture::new(id, uv, layer_idx));
        let primitive = QuadPrimitive::new(quad, self.current_draw_color, texture);
        self.add_quad(primitive);
    }

    pub fn draw_text(
        &mut self,
        position: LogicalPoint<f32>,
        layout: std::sync::Arc<Layout>,
        is_absolute_position: bool,
    ) {
        let primitive = TextPrimitive::new(position, layout);
        self.add_text(primitive, is_absolute_position);
    }

    pub fn add_rect(&mut self, mut rect: RectPrimitive) {
        rect.translate(self.viewport.min.to_vector());

        let clipped_primitive = ClippedPrimitive {
            clip_rect: self.viewport,
            primitive: DrawPrimitive::Rect(rect),
        };

        self.push(clipped_primitive);
    }

    pub fn add_quad(&mut self, mut quad: QuadPrimitive) {
        quad.translate(self.viewport.min.to_vector());

        let clipped_primitive = ClippedPrimitive {
            clip_rect: self.viewport,
            primitive: DrawPrimitive::Quad(quad),
        };

        self.push(clipped_primitive);
    }

    pub fn add_text(&mut self, mut text: TextPrimitive, is_absolute_position: bool) {
        if !is_absolute_position {
            text.translate(self.viewport.min.to_vector());
        };

        let clipped_primitive = ClippedPrimitive {
            clip_rect: self.viewport,
            primitive: DrawPrimitive::Text(text),
        };

        self.push(clipped_primitive);
    }

    #[inline]
    fn push(&mut self, clipped_primitive: ClippedPrimitive) {
        self.layers
            .entry(self.current_layer)
            .or_default()
            .push(clipped_primitive);
    }

    pub fn get_hash(&self) -> u64 {
        calculate_hash(self)
    }
}

impl std::hash::Hash for Layers {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.layers.hash(state);
    }
}
