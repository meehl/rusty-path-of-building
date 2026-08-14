use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
};

use ahash::AHasher;
use euclid::Size2D;
use ordered_float::OrderedFloat;

use crate::{
    color::Srgba,
    dpi::{LogicalQuad, LogicalRect, LogicalSize, NormalizedQuad, NormalizedRect, Uv},
    math::{Point, Quad, Rect},
    renderer::textures::TextureId,
};

#[derive(Copy, Clone, Debug)]
pub struct DrawCommand {
    pub positions: LogicalQuad<f32>,
    pub uvs: NormalizedQuad,
    pub color: Srgba,
    pub texture_id: TextureId,
    pub texture_layer_idx: u32,
    pub clip_rect: LogicalRect<f32>,
}

impl Hash for DrawCommand {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_quad(&self.positions, state);
        hash_quad(&self.uvs, state);
        self.color.hash(state);
        self.texture_id.hash(state);
        self.texture_layer_idx.hash(state);
        hash_rect(&self.clip_rect, state);
    }
}

#[inline]
fn hash_quad<H: Hasher, U>(quad: &Quad<f32, U>, state: &mut H) {
    hash_point(&quad.p0, state);
    hash_point(&quad.p1, state);
    hash_point(&quad.p2, state);
    hash_point(&quad.p3, state);
}

#[inline]
fn hash_rect<H: Hasher, U>(rect: &Rect<f32, U>, state: &mut H) {
    hash_point(&rect.min, state);
    hash_point(&rect.max, state);
}

#[inline]
fn hash_point<H: Hasher, U>(point: &Point<f32, U>, state: &mut H) {
    OrderedFloat(point.x).hash(state);
    OrderedFloat(point.y).hash(state);
}

#[derive(Default)]
pub struct DrawCommandRecorder {
    layers: BTreeMap<(i32, i32), Vec<DrawCommand>>,
    current_layer: (i32, i32),
    current_viewport: LogicalRect<f32>,
    current_draw_color: Srgba,
    hasher: AHasher,
}

impl DrawCommandRecorder {
    fn push(&mut self, cmd: DrawCommand) {
        cmd.hash(&mut self.hasher);
        self.layers.entry(self.current_layer).or_default().push(cmd);
    }

    pub fn reset(&mut self) {
        for layer in self.layers.values_mut() {
            layer.clear();
        }
        self.current_layer = (0, 0);
        self.current_viewport = LogicalRect::from_size(Size2D::new(f32::INFINITY, f32::INFINITY));
        self.current_draw_color = Srgba::WHITE;
        self.hasher = AHasher::default();
    }

    pub fn set_viewport(&mut self, viewport: LogicalRect<f32>) {
        self.current_viewport = viewport;
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

    pub fn draw_image(
        &mut self,
        rect: LogicalRect<f32>,
        texture_id: Option<TextureId>,
        uv: Option<NormalizedRect>,
        layer_idx: u32,
    ) {
        let (texture_id, uvs, texture_layer_idx) = match texture_id {
            Some(texture_id) => (
                texture_id,
                uv.map_or(NormalizedQuad::default_uv(), |uv_rect| uv_rect.into()),
                layer_idx,
            ),
            None => (TextureId::default(), NormalizedQuad::white_uv(), 0),
        };

        self.push(DrawCommand {
            positions: rect.translate(self.current_viewport.min.to_vector()).into(),
            uvs,
            color: self.current_draw_color,
            texture_id,
            texture_layer_idx,
            clip_rect: self.current_viewport,
        });
    }

    pub fn draw_image_quad(
        &mut self,
        quad: LogicalQuad<f32>,
        texture_id: Option<TextureId>,
        uv: Option<NormalizedQuad>,
        layer_idx: u32,
    ) {
        let (texture_id, uvs, texture_layer_idx) = match texture_id {
            Some(texture_id) => (
                texture_id,
                uv.unwrap_or(NormalizedQuad::default_uv()),
                layer_idx,
            ),
            None => (TextureId::default(), NormalizedQuad::white_uv(), 0),
        };

        self.push(DrawCommand {
            positions: quad.translate(self.current_viewport.min.to_vector()),
            uvs,
            color: self.current_draw_color,
            texture_id,
            texture_layer_idx,
            clip_rect: self.current_viewport,
        });
    }

    pub fn draw_glyph(
        &mut self,
        rect: LogicalRect<f32>,
        uv: NormalizedRect,
        color: Srgba,
        layer_idx: u32,
        is_absolute_position: bool,
    ) {
        if !is_absolute_position {
            rect.translate(self.current_viewport.min.to_vector());
        }

        self.push(DrawCommand {
            positions: rect.translate(self.current_viewport.min.to_vector()).into(),
            uvs: uv.into(),
            color,
            // font atlas always lives at default texture ID
            texture_id: TextureId::default(),
            texture_layer_idx: layer_idx,
            clip_rect: self.current_viewport,
        });
    }

    pub fn finish(&mut self) -> (u64, &BTreeMap<(i32, i32), Vec<DrawCommand>>) {
        (self.hasher.finish(), &self.layers)
    }
}
