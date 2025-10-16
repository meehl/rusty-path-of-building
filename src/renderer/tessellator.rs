use crate::{
    dpi::{Normalize, NormalizedQuad, NormalizedRect, Uv},
    fonts::FontAtlasSize,
    renderer::{
        mesh::{ClippedMesh, Mesh},
        primitives::{
            ClippedPrimitive, DrawPrimitive, QuadPrimitive, QuadTexture, RectPrimitive,
            RectTexture, TextPrimitive,
        },
        textures::TextureId,
    },
};

/// Converts [`DrawPrimitive`]s into [`Mesh`]es.
#[derive(Default)]
pub struct Tessellator {
    last_clipped_meshes_size: usize,
}

impl Tessellator {
    pub fn convert_clipped_primitives(
        &mut self,
        clipped_primitives: impl Iterator<Item = ClippedPrimitive>,
        font_atlas_size: FontAtlasSize,
    ) -> Vec<ClippedMesh> {
        profiling::scope!("convert_primitives");

        let mut clipped_meshes = Vec::with_capacity(self.last_clipped_meshes_size);

        for clipped_primitive in clipped_primitives {
            self.convert_clipped_primitive(clipped_primitive, font_atlas_size, &mut clipped_meshes);
        }

        self.last_clipped_meshes_size = clipped_meshes.len();
        clipped_meshes
    }

    pub fn convert_clipped_primitive(
        &mut self,
        clipped_primitive: ClippedPrimitive,
        font_atlas_size: FontAtlasSize,
        out_clipped_meshes: &mut Vec<ClippedMesh>,
    ) {
        let ClippedPrimitive {
            clip_rect,
            primitive,
        } = clipped_primitive;

        if clip_rect.is_empty() {
            return;
        }

        let start_new_mesh = match out_clipped_meshes.last() {
            None => true,
            Some(last_clipped_mesh) => {
                // append to previous mesh if clip_rect and texture_id match.
                // otherwise, start a new mesh.
                !(last_clipped_mesh.clip_rect == clip_rect
                    && last_clipped_mesh.mesh.texture_id == primitive.texture_id())
            }
        };

        if start_new_mesh {
            out_clipped_meshes.push(ClippedMesh {
                clip_rect,
                mesh: Mesh::default(),
            });
        }

        let last_clipped_mesh = out_clipped_meshes.last_mut().unwrap();

        match primitive {
            DrawPrimitive::Rect(rect_primitive) => {
                self.convert_rect_primitive(rect_primitive, &mut last_clipped_mesh.mesh)
            }
            DrawPrimitive::Quad(quad_primitive) => {
                self.convert_quad_primitive(quad_primitive, &mut last_clipped_mesh.mesh)
            }
            DrawPrimitive::Text(text_primitive) => self.convert_text_primitive(
                text_primitive,
                font_atlas_size,
                &mut last_clipped_mesh.mesh,
            ),
        }

        // This can be empty if a new mesh was started but the conversion from a text primitive
        // didn't add any vertices. Our renderer doesn't support empty meshes so remove it
        if last_clipped_mesh.mesh.is_empty() {
            out_clipped_meshes.pop();
        }
    }

    fn convert_rect_primitive(&self, rect_primitive: RectPrimitive, out: &mut Mesh) {
        let RectPrimitive {
            rect,
            color,
            texture,
        } = rect_primitive;

        let (texture_id, uv, layer_idx) = match texture {
            Some(RectTexture {
                texture_id,
                uv,
                layer_idx,
            }) => (texture_id, uv, layer_idx),
            None => (TextureId::default(), NormalizedRect::white_uv(), 0),
        };

        out.add_rect(rect, uv, color, layer_idx);
        out.texture_id = texture_id;
    }

    fn convert_quad_primitive(&self, quad_primitive: QuadPrimitive, out: &mut Mesh) {
        let QuadPrimitive {
            quad,
            color,
            texture,
        } = quad_primitive;

        let (texture_id, uv, layer_idx) = match texture {
            Some(QuadTexture {
                texture_id,
                uv,
                layer_idx,
            }) => (texture_id, uv, layer_idx),
            None => (TextureId::default(), NormalizedQuad::white_uv(), 0),
        };

        out.add_quad(quad, uv, color, layer_idx);
        out.texture_id = texture_id;
    }

    fn convert_text_primitive(
        &self,
        text_primitive: TextPrimitive,
        font_atlas_size: FontAtlasSize,
        out: &mut Mesh,
    ) {
        let TextPrimitive {
            pos: layout_pos,
            layout,
        } = text_primitive;

        if layout.rows.is_empty() {
            return;
        }

        out.vertices.reserve(layout.num_of_vertices);
        out.indices.reserve(layout.num_of_indices);

        // TODO: align to pixel grid?

        for row in &layout.rows {
            for glyph in &row.glyphs {
                let rect = glyph.rect.translate(layout_pos.to_vector());
                let normalized_uv = glyph.uv.normalize(font_atlas_size);
                out.add_rect(rect, normalized_uv, glyph.color, 0);
            }
        }
    }
}
