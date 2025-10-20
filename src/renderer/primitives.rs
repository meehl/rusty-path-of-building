use crate::{
    color::Srgba,
    dpi::{LogicalPoint, LogicalQuad, LogicalRect, LogicalVector, NormalizedQuad, NormalizedRect},
    fonts::Layout,
    math::Point,
    renderer::textures::TextureId,
};
use ordered_float::OrderedFloat;
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

#[derive(Clone)]
pub struct ClippedPrimitive {
    pub clip_rect: LogicalRect<f32>,
    pub primitive: DrawPrimitive,
}

impl Hash for ClippedPrimitive {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_pos(&self.clip_rect.min, state);
        hash_pos(&self.clip_rect.max, state);
        self.primitive.hash(state);
    }
}

#[derive(Clone, Hash)]
pub enum DrawPrimitive {
    Rect(RectPrimitive),
    Quad(QuadPrimitive),
    Text(TextPrimitive),
}

impl DrawPrimitive {
    pub fn texture_id(&self) -> TextureId {
        match self {
            DrawPrimitive::Rect(rect_primitive) => rect_primitive
                .texture
                .map_or_else(TextureId::default, |tex| tex.texture_id),
            DrawPrimitive::Quad(quad_primitive) => quad_primitive
                .texture
                .map_or_else(TextureId::default, |tex| tex.texture_id),
            _ => TextureId::default(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct RectPrimitive {
    pub rect: LogicalRect<f32>,
    pub color: Srgba,
    pub texture: Option<RectTexture>,
}

impl RectPrimitive {
    pub fn new(rect: LogicalRect<f32>, color: Srgba, texture: Option<RectTexture>) -> Self {
        Self {
            rect,
            color,
            texture,
        }
    }

    pub fn translate(&mut self, direction: LogicalVector<f32>) {
        self.rect = self.rect.translate(direction);
    }
}

impl Hash for RectPrimitive {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_pos(&self.rect.min, state);
        hash_pos(&self.rect.max, state);
        self.color.hash(state);
        self.texture.hash(state);
    }
}

#[derive(Clone, Copy)]
pub struct RectTexture {
    pub texture_id: TextureId,
    pub uv: NormalizedRect,
    pub layer_idx: u32,
}

impl RectTexture {
    pub fn new(texture_id: TextureId, uv: NormalizedRect, layer_idx: u32) -> Self {
        Self {
            texture_id,
            uv,
            layer_idx,
        }
    }
}

impl Hash for RectTexture {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.texture_id.hash(state);
        hash_pos(&self.uv.min, state);
        hash_pos(&self.uv.max, state);
        self.layer_idx.hash(state);
    }
}

#[derive(Clone, Copy)]
pub struct QuadPrimitive {
    pub quad: LogicalQuad<f32>,
    pub color: Srgba,
    pub texture: Option<QuadTexture>,
}

impl QuadPrimitive {
    pub fn new(quad: LogicalQuad<f32>, color: Srgba, texture: Option<QuadTexture>) -> Self {
        Self {
            quad,
            color,
            texture,
        }
    }

    pub fn translate(&mut self, direction: LogicalVector<f32>) {
        self.quad = self.quad.translate(direction);
    }
}

impl Hash for QuadPrimitive {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_pos(&self.quad.p0, state);
        hash_pos(&self.quad.p1, state);
        hash_pos(&self.quad.p2, state);
        hash_pos(&self.quad.p3, state);
        self.color.hash(state);
        self.texture.hash(state);
    }
}

#[derive(Clone, Copy)]
pub struct QuadTexture {
    pub texture_id: TextureId,
    pub uv: NormalizedQuad,
    pub layer_idx: u32,
}

impl QuadTexture {
    pub fn new(texture_id: TextureId, uv: NormalizedQuad, layer_idx: u32) -> Self {
        Self {
            texture_id,
            uv,
            layer_idx,
        }
    }
}

impl Hash for QuadTexture {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.texture_id.hash(state);
        hash_pos(&self.uv.p0, state);
        hash_pos(&self.uv.p1, state);
        hash_pos(&self.uv.p2, state);
        hash_pos(&self.uv.p3, state);
        self.layer_idx.hash(state);
    }
}

#[derive(Clone)]
pub struct TextPrimitive {
    pub pos: LogicalPoint<f32>,
    pub layout: Arc<Layout>,
}

impl TextPrimitive {
    pub fn new(pos: LogicalPoint<f32>, layout: Arc<Layout>) -> Self {
        Self { pos, layout }
    }

    #[inline(always)]
    pub fn translate(&mut self, direction: LogicalVector<f32>) {
        self.pos += direction;
    }
}

impl Hash for TextPrimitive {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_pos(&self.pos, state);
        self.layout.hash(state);
    }
}

fn hash_pos<H: Hasher, U>(pos: &Point<f32, U>, state: &mut H) {
    OrderedFloat(pos.x).hash(state);
    OrderedFloat(pos.y).hash(state);
}
