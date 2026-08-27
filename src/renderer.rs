use crate::{
    color::Srgba,
    dpi::{LogicalPoint, LogicalRect, LogicalSize, PhysicalRect, PhysicalSize, ScaleFactor},
    math::Point,
    renderer::{
        image::ImageData,
        textures::{TextureId, TextureOptions, TexturesDelta},
    },
    util::calculate_hash,
    uv::UvPoint,
};
use ahash::HashMap;
use std::{
    borrow::Cow,
    num::{NonZeroU32, NonZeroU64},
    ops::Range,
};
use wgpu::util::DeviceExt;

pub mod image;
mod mipmap;
pub mod textures;

pub const BATCH_TEX_SLOTS: u32 = 32;
const DUMMY_TEXTURE_ID: TextureId = 0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: LogicalPoint<f32>,
    pub uv: UvPoint,
    pub color: Srgba,
    /// Index into batch's texture array
    pub texture_slot: u32,
    /// Index into texture array
    pub layer_idx: u32,
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        // 0: vec2 position
        0 => Float32x2,
        // 1: vec2 uv
        1 => Float32x2,
        // 2: uint color
        2 => Uint32,
        // 3: uint texture_idx
        3 => Uint32,
        // 4: uint layer_idx
        4 => Uint32
    ];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

pub struct Batch {
    pub clip_rect: LogicalRect<f32>,
    pub textures: Vec<TextureId>,
    /// Points into combined index buffer of `RenderJob`
    pub index_range: Range<u32>,
}

pub struct RenderJob {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub batches: Vec<Batch>,
    pub textures_delta: TexturesDelta,
    pub scale_factor: ScaleFactor<f32>,
}

#[derive(Debug)]
struct TextureEntry {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    options: TextureOptions,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    screen_size: LogicalSize<f32>,
}

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,

    index_buffer: Buffer,
    vertex_buffer: Buffer,

    globals_buffer: wgpu::Buffer,
    previous_globals: Globals,
    globals_bind_group: wgpu::BindGroup,
    textures_bind_group_layout: wgpu::BindGroupLayout,

    textures: HashMap<TextureId, TextureEntry>,
    samplers: HashMap<TextureOptions, wgpu::Sampler>,
    batch_bind_group_cache: HashMap<u64, wgpu::BindGroup>,
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        output_color_format: wgpu::TextureFormat,
        output_depth_format: Option<wgpu::TextureFormat>,
    ) -> Self {
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("main_shader_module"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("renderer/main.wgsl"))),
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globals_uniform_buffer"),
            contents: bytemuck::cast_slice(&[Globals {
                screen_size: LogicalSize::zero(),
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let globals_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("globals_uniform_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(std::mem::size_of::<Globals>() as _),
                        ty: wgpu::BufferBindingType::Uniform,
                    },
                    count: None,
                }],
            });

        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals_uniform_bind_group"),
            layout: &globals_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let textures_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                        },
                        count: Some(NonZeroU32::new(BATCH_TEX_SLOTS).unwrap()),
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: Some(NonZeroU32::new(BATCH_TEX_SLOTS).unwrap()),
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[
                Some(&globals_bind_group_layout),
                Some(&textures_bind_group_layout),
            ],
            immediate_size: 0,
        });

        let depth_stencil = output_depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: None,
            depth_compare: None,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                entry_point: Some("vs_main"),
                module: &shader_module,
                buffers: &[Some(Vertex::desc())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                unclipped_depth: false,
                conservative: false,
                cull_mode: None,
                front_face: wgpu::FrontFace::default(),
                polygon_mode: wgpu::PolygonMode::default(),
                strip_index_format: None,
            },
            depth_stencil,
            multisample: wgpu::MultisampleState {
                alpha_to_coverage_enabled: false,
                count: 1,
                mask: !0,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview_mask: None,
            cache: None,
        });

        let vertex_buffer = Buffer::new(
            device,
            wgpu::BufferUsages::VERTEX,
            NonZeroU64::new(2048).expect("2048 is non-zero"),
            NonZeroU64::new(std::mem::size_of::<Vertex>() as u64)
                .expect("size of vertex is non-zero"),
        );
        let index_buffer = Buffer::new(
            device,
            wgpu::BufferUsages::INDEX,
            NonZeroU64::new(2048 * 3).expect("2048 * 3 is non-zero"),
            NonZeroU64::new(std::mem::size_of::<u32>() as u64).expect("size of u32 is non-zero"),
        );

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            globals_buffer: uniform_buffer,
            previous_globals: Globals {
                screen_size: LogicalSize::zero(),
            },
            globals_bind_group,
            textures_bind_group_layout,
            textures: HashMap::default(),
            samplers: HashMap::default(),
            batch_bind_group_cache: Default::default(),
        }
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        render_pass: &mut wgpu::RenderPass<'static>,
        render_job: &RenderJob,
        screen_size: PhysicalSize<u32>,
        scale_factor: ScaleFactor<f32>,
    ) {
        profiling::scope!("render");

        let screen_rect = PhysicalRect::from_origin_and_size(Point::zero(), screen_size);
        render_pass.set_viewport(
            0.0,
            0.0,
            screen_size.width as f32,
            screen_size.height as f32,
            0.0,
            1.0,
        );
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.globals_bind_group, &[]);
        render_pass.set_index_buffer(
            self.index_buffer.buffer.slice(..),
            wgpu::IndexFormat::Uint32,
        );
        render_pass.set_vertex_buffer(0, self.vertex_buffer.buffer.slice(..));

        for batch in &render_job.batches {
            // set textures array bind group
            let bind_group = self.get_or_build_batch_bind_group(device, batch);
            render_pass.set_bind_group(1, bind_group, &[]);

            // scissor
            let phys_clip_rect = batch.clip_rect * scale_factor;
            let scissor = phys_clip_rect
                .intersection(&screen_rect.cast())
                .map(|s| s.round().to_u32());

            // skip batch if scissor doesn't intersect screen
            let Some(scissor) = scissor else {
                continue;
            };

            render_pass.set_scissor_rect(
                scissor.min.x,
                scissor.min.y,
                scissor.width(),
                scissor.height(),
            );

            // draw batch
            render_pass.draw_indexed(batch.index_range.clone(), 0, 0..1);
        }

        render_pass.set_scissor_rect(0, 0, screen_size.width, screen_size.height);
    }

    /// Uploads texture data.
    /// Needs to be called before [`Self::render`].
    pub fn update_textures(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        textures_delta: &TexturesDelta,
    ) {
        profiling::scope!("update_textures");

        if !textures_delta.update.is_empty() {
            self.batch_bind_group_cache.clear();
        }

        for (id, image_delta) in &textures_delta.update {
            let ImageData {
                format,
                width,
                height,
                array_layers,
                mipmap_count,
                data_order,
                ref bytes,
            } = image_delta.image;

            let size = wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: array_layers,
            };

            // only generate mipmaps for uncompressed images that don't already have mipmaps
            let gen_mipmaps = image_delta.options.generate_mipmaps
                && mipmap_count.get() == 1
                && !format.is_compressed();

            let mip_level_count = if gen_mipmaps {
                size.max_mips(wgpu::TextureDimension::D2)
            } else {
                mipmap_count.get()
            };

            let label_str = format!("texture_{id:?}");
            let label = Some(label_str.as_str());

            let texture = create_texture_with_data(
                device,
                queue,
                &wgpu::TextureDescriptor {
                    label,
                    size,
                    mip_level_count,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[format.add_srgb_suffix()],
                },
                data_order.into(),
                bytes,
                gen_mipmaps,
            );

            if gen_mipmaps {
                mipmap::generate_mipmap_chain(queue, &texture, bytes);
            }

            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });

            self.samplers
                .entry(image_delta.options)
                .or_insert_with(|| create_sampler(image_delta.options, device));

            self.textures.insert(
                *id,
                TextureEntry {
                    texture,
                    view,
                    options: image_delta.options,
                },
            );
        }
    }

    pub fn free_textures(&mut self, textures_delta: &TexturesDelta) {
        profiling::scope!("free_textures");

        if !textures_delta.free.is_empty() {
            self.batch_bind_group_cache.clear();
        }

        for id in &textures_delta.free {
            if let Some(texture) = self.textures.remove(id) {
                texture.texture.destroy();
            }
        }
    }

    /// Updates the vertex, index, and uniform buffers.
    /// Needs to be called before [`Self::render`].
    pub fn update_buffers(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_job: &RenderJob,
        screen_size: PhysicalSize<u32>,
        scale_factor: ScaleFactor<f32>,
    ) {
        profiling::scope!("update_buffers");

        let uniform_buffer_content = Globals {
            screen_size: screen_size.cast() / scale_factor,
        };

        // update globals uniform buffer
        if uniform_buffer_content != self.previous_globals {
            queue.write_buffer(
                &self.globals_buffer,
                0,
                bytemuck::cast_slice(&[uniform_buffer_content]),
            );
            self.previous_globals = uniform_buffer_content;
        }

        let vertex_count = render_job.vertices.len();
        let index_count = render_job.indices.len();

        if !(index_count > 0 && vertex_count > 0) {
            return;
        }

        // update index and vertex buffers
        let mut staging_index_buffer = self.index_buffer.create_staging_buffer(
            device,
            queue,
            NonZeroU64::new(index_count as u64).expect("index_count > 0"),
        );
        let mut staging_vertex_buffer = self.vertex_buffer.create_staging_buffer(
            device,
            queue,
            NonZeroU64::new(vertex_count as u64).expect("vertex_count > 0"),
        );

        staging_index_buffer
            .slice(..)
            .copy_from_slice(bytemuck::cast_slice(&render_job.indices));

        staging_vertex_buffer
            .slice(..)
            .copy_from_slice(bytemuck::cast_slice(&render_job.vertices));
    }

    fn get_or_build_batch_bind_group(
        &mut self,
        device: &wgpu::Device,
        batch: &Batch,
    ) -> &wgpu::BindGroup {
        let key = calculate_hash(&batch.textures);

        self.batch_bind_group_cache.entry(key).or_insert_with(|| {
            let mut views: Vec<_> = batch
                .textures
                .iter()
                .map(|id| &self.textures[id].view)
                .collect();
            let mut samplers: Vec<_> = batch
                .textures
                .iter()
                .map(|id| {
                    self.samplers
                        .get(&self.textures[id].options)
                        .expect("sampler is present from update_textures")
                })
                .collect();

            // Metal doesn't support the PARTIALLY_BOUND_BINDING_ARRAY feature
            // so we pad to up MAX_SLOTS with a dummy view/sampler
            let dummy = &self.textures[&DUMMY_TEXTURE_ID];
            let dummy_sampler = self.samplers.get(&dummy.options).unwrap();
            views.resize(BATCH_TEX_SLOTS as usize, &dummy.view);
            samplers.resize(BATCH_TEX_SLOTS as usize, dummy_sampler);

            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("batch_textures_bind_group"),
                layout: &self.textures_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureViewArray(&views),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::SamplerArray(&samplers),
                    },
                ],
            })
        })
    }
}

struct Buffer {
    buffer: wgpu::Buffer,
    size: wgpu::BufferSize,
    usage: wgpu::BufferUsages,
    stride: NonZeroU64,
}

impl Buffer {
    fn new(
        device: &wgpu::Device,
        usage: wgpu::BufferUsages,
        start_capacity: NonZeroU64,
        stride: NonZeroU64,
    ) -> Self {
        let size = start_capacity.checked_mul(stride).unwrap();

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            size: size.get(),
            mapped_at_creation: false,
        });

        Self {
            buffer,
            size,
            usage,
            stride,
        }
    }

    fn create_staging_buffer(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        required_capacity: NonZeroU64,
    ) -> wgpu::QueueWriteBufferView {
        let required_size = required_capacity.checked_mul(self.stride).unwrap();

        // resize buffer to required size
        if self.size < required_size {
            self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                usage: self.usage | wgpu::BufferUsages::COPY_DST,
                size: required_size.get(),
                mapped_at_creation: false,
            });
            self.size = required_size;
        }

        let staging_buffer = queue.write_buffer_with(&self.buffer, 0, required_size);
        let Some(staging_buffer) = staging_buffer else {
            panic!("Failed to create staging buffer!");
        };
        staging_buffer
    }
}

fn create_sampler(options: TextureOptions, device: &wgpu::Device) -> wgpu::Sampler {
    let TextureOptions {
        magnification,
        minification,
        wrap_mode,
        mipmap_mode,
        ..
    } = options;
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some(&format!(
            "sampler (mag: {magnification:?}, min {minification:?})"
        )),
        mag_filter: magnification,
        min_filter: minification,
        address_mode_u: wrap_mode,
        address_mode_v: wrap_mode,
        mipmap_filter: mipmap_mode,
        ..Default::default()
    })
}

/// Adapted from `wgpu::Device::create_texture_with_data`.
/// Doesn't upload any data for mip level > 0 if `skip_mipmaps` is true.
fn create_texture_with_data(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    desc: &wgpu::TextureDescriptor<'_>,
    order: wgpu::wgt::TextureDataOrder,
    data: &[u8],
    skip_mipmaps: bool,
) -> wgpu::Texture {
    // Implicitly add the COPY_DST usage
    let mut desc = desc.to_owned();
    desc.usage |= wgpu::TextureUsages::COPY_DST;
    let texture = device.create_texture(&desc);

    // Will return None only if it's a combined depth-stencil format
    // If so, default to 4, validation will fail later anyway since the depth or stencil
    // aspect needs to be written to individually
    let block_size = desc.format.block_copy_size(None).unwrap_or(4);
    let (block_width, block_height) = desc.format.block_dimensions();
    let layer_iterations = desc.array_layer_count();

    let outer_iteration;
    let inner_iteration;
    match order {
        wgpu::wgt::TextureDataOrder::LayerMajor => {
            outer_iteration = layer_iterations;
            inner_iteration = desc.mip_level_count;
        }
        wgpu::wgt::TextureDataOrder::MipMajor => {
            outer_iteration = desc.mip_level_count;
            inner_iteration = layer_iterations;
        }
    }

    let mut binary_offset = 0;
    for outer in 0..outer_iteration {
        for inner in 0..inner_iteration {
            let (layer, mip) = match order {
                wgpu::wgt::TextureDataOrder::LayerMajor => (outer, inner),
                wgpu::wgt::TextureDataOrder::MipMajor => (inner, outer),
            };

            let mut mip_size = desc.mip_level_size(mip).unwrap();
            // copying layers separately
            if desc.dimension != wgpu::wgt::TextureDimension::D3 {
                mip_size.depth_or_array_layers = 1;
            }

            // When uploading mips of compressed textures and the mip is supposed to be
            // a size that isn't a multiple of the block size, the mip needs to be uploaded
            // as its "physical size" which is the size rounded up to the nearest block size.
            let mip_physical = mip_size.physical_size(desc.format);

            // All these calculations are performed on the physical size as that's the
            // data that exists in the buffer.
            let width_blocks = mip_physical.width / block_width;
            let height_blocks = mip_physical.height / block_height;

            let bytes_per_row = width_blocks * block_size;
            let data_size = bytes_per_row * height_blocks * mip_size.depth_or_array_layers;

            let end_offset = binary_offset + data_size as usize;

            if mip == 0 || !skip_mipmaps {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: mip,
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: 0,
                            z: layer,
                        },
                        aspect: wgpu::wgt::TextureAspect::All,
                    },
                    &data[binary_offset..end_offset],
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row),
                        rows_per_image: Some(height_blocks),
                    },
                    mip_physical,
                );
            }

            binary_offset = end_offset;
        }
    }

    texture
}
