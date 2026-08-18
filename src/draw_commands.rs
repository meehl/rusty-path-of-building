use std::{collections::BTreeMap, hash::Hasher};

use ahash::AHasher;
use euclid::Size2D;

use crate::{
    color::Srgba,
    dpi::{LogicalQuad, LogicalRect, LogicalSize, NormalizedQuad, NormalizedRect, Uv},
    renderer::textures::TextureId,
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DrawCommand {
    pub positions: LogicalQuad<f32>,
    pub uvs: NormalizedQuad,
    pub texture_id: TextureId,
    pub clip_rect: LogicalRect<f32>,
    pub texture_layer_idx: u32,
    pub color: Srgba,
}

impl DrawCommand {
    #[inline(always)]
    fn hash_into<H: Hasher>(&self, state: &mut H) {
        state.write(bytemuck::bytes_of(self));
    }
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
        cmd.hash_into(&mut self.hasher);
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
        let rect = if is_absolute_position {
            rect
        } else {
            rect.translate(self.current_viewport.min.to_vector())
        };

        self.push(DrawCommand {
            positions: rect.into(),
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
