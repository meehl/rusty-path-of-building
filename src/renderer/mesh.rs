use crate::{
    color::Srgba,
    dpi::{
        LogicalPoint, LogicalQuad, LogicalRect, NormalizedPoint, NormalizedQuad, NormalizedRect,
    },
    math::Corners,
    renderer::textures::TextureId,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: LogicalPoint<f32>,
    pub uv: NormalizedPoint,
    pub color: Srgba,
    /// Index into texture array
    /// TODO: Remove from Vertex and put into Mesh. Use push constant to set index
    /// before each draw call. Not sure if actually faster, profiling needed.
    pub layer_idx: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub texture_id: TextureId,
}

impl Mesh {
    #[inline]
    pub fn add_rect(
        &mut self,
        rect: LogicalRect<f32>,
        uv: NormalizedRect,
        color: Srgba,
        layer_idx: u32,
    ) {
        let i = self.vertices.len() as u32;
        self.indices
            .extend_from_slice(&[i, i + 1, i + 3, i + 1, i + 2, i + 3]);

        self.vertices.extend_from_slice(&[
            Vertex {
                pos: rect.top_left(),
                uv: uv.top_left(),
                color,
                layer_idx,
            },
            Vertex {
                pos: rect.top_right(),
                uv: uv.top_right(),
                color,
                layer_idx,
            },
            Vertex {
                pos: rect.bottom_right(),
                uv: uv.bottom_right(),
                color,
                layer_idx,
            },
            Vertex {
                pos: rect.bottom_left(),
                uv: uv.bottom_left(),
                color,
                layer_idx,
            },
        ]);
    }

    #[inline]
    pub fn add_quad(
        &mut self,
        quad: LogicalQuad<f32>,
        uv: NormalizedQuad,
        color: Srgba,
        layer_idx: u32,
    ) {
        let i = self.vertices.len() as u32;
        self.indices
            .extend_from_slice(&[i, i + 1, i + 3, i + 1, i + 2, i + 3]);

        self.vertices.extend_from_slice(&[
            Vertex {
                pos: quad.p0,
                uv: uv.p0,
                color,
                layer_idx,
            },
            Vertex {
                pos: quad.p1,
                uv: uv.p1,
                color,
                layer_idx,
            },
            Vertex {
                pos: quad.p2,
                uv: uv.p2,
                color,
                layer_idx,
            },
            Vertex {
                pos: quad.p3,
                uv: uv.p3,
                color,
                layer_idx,
            },
        ]);
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() && self.indices.is_empty()
    }
}

pub struct ClippedMesh {
    // Only parts of the mesh that intersect with this will be rendered
    pub clip_rect: LogicalRect<f32>,
    pub mesh: Mesh,
}
