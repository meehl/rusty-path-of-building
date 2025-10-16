use std::collections::BTreeMap;

use crate::{
    dpi::LogicalRect,
    math::Rect,
    renderer::primitives::{
        ClippedPrimitive, DrawPrimitive, QuadPrimitive, RectPrimitive, TextPrimitive,
    },
};

/// Holds the draw primitives for each layer.
///
/// Adding a primitive places it in currently set layer. Positions are interpreted as being relative to
/// the current viewport. They are translated into absolute positions (screen positions) and
/// clipped by the viewport.
pub struct Layers {
    layers: BTreeMap<(i32, i32), Vec<ClippedPrimitive>>,
    current_layer: (i32, i32),
    viewport: LogicalRect<f32>,
}

impl Layers {
    pub fn new() -> Self {
        Self {
            layers: Default::default(),
            current_layer: (0, 0),
            viewport: Rect::zero(),
        }
    }

    pub fn reset(&mut self) {
        self.current_layer = (0, 0);
        self.layers.clear();
    }

    /// Consume primitives and return an iterator over them in drawing order.
    pub fn consume_layers(&mut self) -> impl Iterator<Item = ClippedPrimitive> {
        let layers = std::mem::take(&mut self.layers);
        layers.into_values().flatten()
    }

    pub fn set_viewport(&mut self, viewport: LogicalRect<f32>) {
        self.viewport = viewport;
    }

    pub fn set_draw_layer(&mut self, layer: i32, sublayer: i32) {
        self.current_layer = (layer, sublayer);
    }

    pub fn set_draw_sublayer(&mut self, sublayer: i32) {
        self.set_draw_layer(self.current_layer.0, sublayer);
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
}

impl std::hash::Hash for Layers {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.layers.hash(state);
    }
}
