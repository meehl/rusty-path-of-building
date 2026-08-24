use ahash::HashMap;

use crate::{
    dpi::LogicalRect,
    draw_commands::DrawCommand,
    renderer::{
        Batch, RenderJob, Vertex,
        textures::{TextureId, TexturesDelta},
    },
};

struct BatchBuilder {
    textures: Vec<TextureId>,
    // maps TextureId to index into textures array
    texture_mapping: HashMap<TextureId, u32>,
    clip_rect: LogicalRect<f32>,
    // index into batch's indecies array
    start_index: u32,
}

impl BatchBuilder {
    fn new(clip_rect: LogicalRect<f32>, start_index: u32) -> Self {
        // seed slot 0 with font atlas.
        // the font atlas texture is used for solid color and text
        // rendering so it needs to be present in almost every batch.
        let textures = vec![TextureId::default()];
        let texture_mapping = HashMap::from_iter([(TextureId::default(), 0)]);
        Self {
            textures,
            texture_mapping,
            clip_rect,
            start_index,
        }
    }

    /// Tries to assign a texture to the batch.
    ///
    /// Returns slot_index into textures array if texture can be assigned.
    /// Returns `None` if maximum amount of texture slots is reached.
    fn try_assign(&mut self, id: TextureId, max_slots: u32) -> Option<u32> {
        if let Some(&slot) = self.texture_mapping.get(&id) {
            return Some(slot);
        }

        if self.textures.len() as u32 >= max_slots {
            return None;
        }

        let slot = self.textures.len() as u32;
        self.textures.push(id);
        self.texture_mapping.insert(id, slot);
        Some(slot)
    }

    fn finish(self, end_index: u32) -> Option<Batch> {
        if self.start_index < end_index {
            Some(Batch {
                clip_rect: self.clip_rect,
                textures: self.textures,
                index_range: self.start_index..end_index,
            })
        } else {
            None
        }
    }
}

pub fn build_render_job(
    commands: &[DrawCommand],
    textures_delta: TexturesDelta,
    scale_factor: f32,
    max_slots: u32,
) -> RenderJob {
    profiling::scope!("build_render_job");

    let mut vertices = Vec::with_capacity(commands.len() * 4);
    let mut indices = Vec::with_capacity(commands.len() * 6);
    let mut batches = Vec::new();

    let Some(first_cmd) = commands.first() else {
        // no draw commands, only texture updates
        return RenderJob {
            vertices,
            indices,
            batches,
            textures_delta,
            scale_factor,
        };
    };

    let mut builder = BatchBuilder::new(first_cmd.clip_rect, 0);

    for cmd in commands {
        // start new batch if clip_rects don't match
        if cmd.clip_rect != builder.clip_rect {
            let next_builder = BatchBuilder::new(cmd.clip_rect, indices.len() as u32);
            if let Some(batch) =
                std::mem::replace(&mut builder, next_builder).finish(indices.len() as u32)
            {
                batches.push(batch);
            }
        }

        let slot = builder
            .try_assign(cmd.texture_id, max_slots)
            .unwrap_or_else(|| {
                let next_builder = BatchBuilder::new(cmd.clip_rect, indices.len() as u32);
                if let Some(batch) =
                    std::mem::replace(&mut builder, next_builder).finish(indices.len() as u32)
                {
                    batches.push(batch);
                }

                builder
                    .try_assign(cmd.texture_id, max_slots)
                    .expect("new batch always has enough space")
            });

        let i = vertices.len() as u32;

        indices.extend_from_slice(&[i, i + 1, i + 3, i + 1, i + 2, i + 3]);

        vertices.extend_from_slice(&[
            Vertex {
                pos: cmd.positions.p0,
                uv: cmd.uvs.p0,
                color: cmd.color,
                texture_slot: slot,
                layer_idx: cmd.texture_layer_idx,
            },
            Vertex {
                pos: cmd.positions.p1,
                uv: cmd.uvs.p1,
                color: cmd.color,
                texture_slot: slot,
                layer_idx: cmd.texture_layer_idx,
            },
            Vertex {
                pos: cmd.positions.p2,
                uv: cmd.uvs.p2,
                color: cmd.color,
                texture_slot: slot,
                layer_idx: cmd.texture_layer_idx,
            },
            Vertex {
                pos: cmd.positions.p3,
                uv: cmd.uvs.p3,
                color: cmd.color,
                texture_slot: slot,
                layer_idx: cmd.texture_layer_idx,
            },
        ]);
    }

    if let Some(batch) = builder.finish(indices.len() as u32) {
        batches.push(batch);
    }

    RenderJob {
        vertices,
        indices,
        batches,
        textures_delta,
        scale_factor,
    }
}
