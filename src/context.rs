use crate::{
    Game,
    color::Srgba,
    dpi::{
        ConvertToLogical, LogicalPoint, LogicalQuad, LogicalRect, LogicalSize, NormalizedQuad,
        NormalizedRect, PhysicalRect, PhysicalSize,
    },
    fonts::{Fonts, LayoutJob},
    input::InputState,
    layers::Layers,
    renderer::{
        mesh::ClippedMesh,
        primitives::{QuadPrimitive, QuadTexture, RectPrimitive, RectTexture, TextPrimitive},
        tessellator::Tessellator,
        textures::{TextureId, TexturesDelta, WrappedTextureManager},
    },
    util::{calculate_hash, change_working_directory},
};
use arboard::Clipboard;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    sync::Arc,
};
use winit::{event::WindowEvent, window::Window};

// NOTE: This looks ugly but i haven't found a better way to do it yet.
// The C functions used for rendering need a way to access the context.
// They can't be defined as closures like all other lua callback functions
// and we can't change their arguments. The only way for them to access
// something outside their scope is to have some kind of static variable.
thread_local! {
    pub static CONTEXT: RefCell<Context> = RefCell::new(Context::new());
}

pub struct Context {
    window: Option<Arc<Window>>,
    screen_size: PhysicalSize<u32>,
    pixels_per_point: f32,
    pub input_state: InputState,
    pub clipboard: Clipboard,
    pub texture_manager: WrappedTextureManager,
    tessellator: Tessellator,
    fonts: Fonts,
    layers: Layers,
    current_draw_color: Srgba,
    previous_layers_hash: u64,
    current_working_dir: PathBuf,
    pub needs_restart: bool,
    pub force_render: bool,
}

impl Context {
    pub fn new() -> Self {
        Self {
            window: None,
            screen_size: PhysicalSize::new(0, 0),
            pixels_per_point: 1.0,
            input_state: InputState::default(),
            clipboard: Clipboard::new().unwrap(),
            texture_manager: WrappedTextureManager::new(),
            tessellator: Tessellator::default(),
            fonts: Fonts::new(),
            layers: Layers::new(),
            current_draw_color: Srgba::WHITE,
            previous_layers_hash: 0,
            current_working_dir: Game::script_dir().clone(),
            needs_restart: false,
            force_render: false,
        }
    }

    pub fn begin_frame(&mut self) {
        profiling::scope!("begin_frame");

        self.fonts.begin_frame();
        self.layers.reset();
        self.reset_viewport();
    }

    pub fn end_frame(&mut self) -> FrameOutput {
        profiling::scope!("end_frame");

        let font_atlas_size = self.fonts.font_atlas().size();
        if let Some(font_image_delta) = self.fonts.font_atlas_delta() {
            self.texture_manager.update_font_texture(font_image_delta);
        }

        let textures_delta = self.texture_manager.take_delta();

        // Skip rendering this frame if identical to last one and no textures changed
        let layers_hash = calculate_hash(&self.layers);
        let elide_frame = layers_hash == self.previous_layers_hash && textures_delta.is_empty();

        let render_job = if elide_frame && !self.force_render {
            RenderJob::Skip
        } else {
            self.force_render = false;
            self.previous_layers_hash = layers_hash;

            let primitives = self.layers.consume_layers();
            let meshes = self.tessellator.convert_clipped_primitives(
                primitives,
                font_atlas_size,
                self.pixels_per_point,
            );

            RenderJob::Render {
                meshes,
                textures_delta,
            }
        };

        FrameOutput { render_job }
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                self.screen_size = PhysicalSize::new(size.width, size.height);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.pixels_per_point = *scale_factor as f32;
            }
            _ => {}
        }

        self.input_state.on_window_event(event);
    }

    pub fn screen_size_logical(&self) -> LogicalSize<u32> {
        self.screen_size.to_logical(self.pixels_per_point)
    }

    pub fn mouse_pos_logical(&self) -> LogicalPoint<f32> {
        self.input_state
            .mouse_pos()
            .to_logical(self.pixels_per_point)
    }

    pub fn pixels_per_point(&self) -> f32 {
        self.pixels_per_point
    }

    pub fn current_working_dir(&self) -> &Path {
        &self.current_working_dir
    }

    pub fn set_current_working_dir(&mut self, path: PathBuf) {
        if change_working_directory(&path).is_ok() {
            self.current_working_dir = path;
        }
    }

    pub fn set_window(&mut self, window: Arc<Window>) {
        self.window = Some(window);
    }

    pub fn set_window_title(&self, title: &str) {
        if let Some(ref window) = self.window {
            window.set_title(title);
        }
    }
}

pub enum RenderJob {
    Render {
        meshes: Vec<ClippedMesh>,
        textures_delta: TexturesDelta,
    },
    Skip,
}

pub struct FrameOutput {
    pub render_job: RenderJob,
}

impl Context {
    pub fn set_draw_layer(&mut self, layer: i32, sublayer: i32) {
        self.layers.set_draw_layer(layer, sublayer);
    }

    pub fn set_draw_sublayer(&mut self, sublayer: i32) {
        self.layers.set_draw_sublayer(sublayer);
    }

    pub fn set_viewport(&mut self, viewport: LogicalRect<f32>) {
        self.layers.set_viewport(viewport);
    }

    pub fn reset_viewport(&mut self) {
        let viewport = PhysicalRect::from_size(self.screen_size);
        self.layers
            .set_viewport(viewport.to_logical(self.pixels_per_point));
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
        self.layers.add_rect(primitive);
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
        self.layers.add_quad(primitive);
    }

    pub fn draw_text(
        &mut self,
        position: LogicalPoint<f32>,
        job: LayoutJob,
        is_absolute_position: bool,
    ) {
        let layout = self.fonts.layout(job, self.pixels_per_point as f32);
        let primitive = TextPrimitive::new(position, layout);
        self.layers.add_text(primitive, is_absolute_position);
    }

    /// Width of laid out text
    pub fn get_text_width(&mut self, job: LayoutJob) -> i32 {
        let layout = self.fonts.layout(job, self.pixels_per_point as f32);
        layout.width() as i32
    }

    /// Text index based on where cursor is positioned within text
    pub fn get_text_index_at_cursor(&mut self, job: LayoutJob, cursor: LogicalPoint<f32>) -> usize {
        let layout = self.fonts.layout(job, self.pixels_per_point as f32);
        layout.cursor_index(cursor)
    }
}
