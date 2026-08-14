use crate::{
    draw_commands::DrawCommand,
    renderer::mesh::{ClippedMesh, Mesh},
};

/// Converts [`DrawPrimitive`]s into [`Mesh`]es.
#[derive(Default)]
pub struct Tessellator {
    last_clipped_meshes_size: usize,
}

impl Tessellator {
    pub fn convert_draw_commands(&mut self, draw_commands: Vec<DrawCommand>) -> Vec<ClippedMesh> {
        profiling::scope!("convert_primitives");

        let mut clipped_meshes = Vec::with_capacity(self.last_clipped_meshes_size);

        for cmd in draw_commands {
            self.convert_draw_command(cmd, &mut clipped_meshes);
        }

        self.last_clipped_meshes_size = clipped_meshes.len();
        clipped_meshes
    }

    pub fn convert_draw_command(
        &mut self,
        cmd: DrawCommand,
        out_clipped_meshes: &mut Vec<ClippedMesh>,
    ) {
        let DrawCommand {
            positions,
            uvs,
            color,
            texture_id,
            texture_layer_idx,
            clip_rect,
        } = cmd;

        if clip_rect.is_empty() {
            return;
        }

        let start_new_mesh = out_clipped_meshes.last().is_none_or(|last_clipped_mesh| {
            // append to previous mesh if clip_rect and texture_id match.
            // otherwise, start a new mesh.
            !(last_clipped_mesh.clip_rect == clip_rect
                && last_clipped_mesh.mesh.texture_id == texture_id)
        });

        if start_new_mesh {
            out_clipped_meshes.push(ClippedMesh {
                clip_rect,
                mesh: Mesh::default(),
            });
        }

        let last_clipped_mesh = out_clipped_meshes.last_mut().unwrap();

        last_clipped_mesh
            .mesh
            .add_quad(positions, uvs, color, texture_layer_idx);
        last_clipped_mesh.mesh.texture_id = texture_id;

        // This can be empty if a new mesh was started but the conversion from a text primitive
        // didn't add any vertices. Our renderer doesn't support empty meshes so remove it
        if last_clipped_mesh.mesh.is_empty() {
            out_clipped_meshes.pop();
        }
    }
}
