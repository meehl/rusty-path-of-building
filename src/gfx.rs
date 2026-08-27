use crate::{
    dpi::PhysicalSize,
    renderer::{RenderJob, Renderer},
};
use std::sync::Arc;
use wgpu::{Texture, TextureFormat, TextureView};
use winit::window::Window;

const REQUIRED_TEXTURE_LAYERS: u32 = 1024;
const REQUIRED_TEXTURE_ARRAY_SLOTS: u32 = 64;

pub struct GraphicsContext {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    is_surface_configured: bool,
    renderer: Renderer,
    blit_texture: wgpu::Texture,
    blit_texture_view: wgpu::TextureView,
    texture_blitter: wgpu::util::TextureBlitter,
    pub window: Arc<Window>,
}

impl GraphicsContext {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });

        let surface = instance.create_surface(Arc::clone(&window)).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: true,
            })
            .await?;

        let required_features = wgpu::Features::TEXTURE_COMPRESSION_BC
            | wgpu::Features::TEXTURE_BINDING_ARRAY
            | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING;
        let required_limits = wgpu::Limits {
            max_texture_array_layers: REQUIRED_TEXTURE_LAYERS,
            max_binding_array_elements_per_shader_stage: REQUIRED_TEXTURE_ARRAY_SLOTS,
            max_binding_array_sampler_elements_per_shader_stage: REQUIRED_TEXTURE_ARRAY_SLOTS,
            ..Default::default()
        };

        if !adapter.features().contains(required_features) {
            anyhow::bail!(
                "Unsupported features were requested: {}",
                required_features - adapter.features()
            );
        }

        let mut failed_limit = Vec::new();

        required_limits.check_limits_with_fail_fn(
            &adapter.limits(),
            false,
            |name, requested, allowed| {
                failed_limit.push((name, requested, allowed));
            },
        );

        if let Some((name, requested, allowed)) = failed_limit.pop() {
            anyhow::bail!(
                "Requested limit '{name}' value {requested} is better than allowed {allowed}!"
            )
        }

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features,
                required_limits,
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
                experimental_features: Default::default(),
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);

        // NOTE: PoB incorrectly performs mixing and blending in sRGB space.
        // To get a similar visual outcome, we need to do the same.
        // Select a non-sRGB format so that no automatic linear -> sRGB conversion
        // is performed.
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| !f.is_srgb() && f.required_features().is_empty())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };

        let (blit_texture, blit_texture_view) =
            create_blit_texture(&device, config.width, config.height, config.format);

        let texture_blitter = wgpu::util::TextureBlitter::new(&device, config.format);

        let renderer = Renderer::new(&device, config.format, None);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_surface_configured: false,
            renderer,
            blit_texture,
            blit_texture_view,
            texture_blitter,
            window,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.is_surface_configured = true;

            (self.blit_texture, self.blit_texture_view) =
                create_blit_texture(&self.device, width, height, self.config.format);
        }
    }

    pub fn render(&mut self, render_job: Option<RenderJob>) -> anyhow::Result<()> {
        profiling::scope!("render");

        if !self.is_surface_configured {
            return Ok(());
        }

        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_tex) => surface_tex,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_tex) => surface_tex,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                // Skip frame
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
        };

        let surface_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // If render_job is `None`, skip rendering and just blit the texture
        // of the previous frame onto the surface texture.
        if let Some(render_job) = render_job {
            let screen_size = PhysicalSize::new(self.config.width, self.config.height);

            // upload new textures
            self.renderer
                .update_textures(&self.device, &self.queue, &render_job.textures_delta);

            // upload vertex, index, and uniform buffers
            self.renderer.update_buffers(
                &self.device,
                &self.queue,
                &render_job,
                screen_size,
                render_job.scale_factor,
            );

            let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.blit_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                label: Some("main render pass"),
                occlusion_query_set: None,
                multiview_mask: None,
            });

            self.renderer.render(
                &self.device,
                &mut rpass.forget_lifetime(),
                &render_job,
                screen_size,
                render_job.scale_factor,
            );

            self.renderer.free_textures(&render_job.textures_delta);
        }

        {
            profiling::scope!("blit");
            self.texture_blitter.copy(
                &self.device,
                &mut encoder,
                &self.blit_texture_view,
                &surface_view,
            );
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        self.window.pre_present_notify();
        self.queue.present(output);

        Ok(())
    }
}

fn create_blit_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: TextureFormat,
) -> (Texture, TextureView) {
    let blit_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Blit Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let blit_texture_view = blit_texture.create_view(&wgpu::TextureViewDescriptor::default());

    (blit_texture, blit_texture_view)
}
