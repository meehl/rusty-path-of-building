use crate::{
    dpi::{ConvertToLogical, ConvertToPhysical, LogicalSize, PhysicalRect, PhysicalSize},
    math::Point,
    renderer::{
        image::ImageData,
        mesh::{ClippedMesh, Vertex},
        textures::{TextureId, TextureOptions, TexturesDelta},
    },
};
use ahash::HashMap;
use std::{borrow::Cow, num::NonZeroU64, ops::Range};
use wgpu::util::DeviceExt;

pub mod image;
pub mod mesh;
mod mipmap;
pub mod primitives;
pub mod tessellator;
pub mod textures;

#[derive(Debug)]
struct Texture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    screen_size: LogicalSize<f32>,
}

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,

    index_buffer: SlicedBuffer,
    vertex_buffer: SlicedBuffer,

    globals_buffer: wgpu::Buffer,
    previous_globals: Globals,
    globals_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,

    textures: HashMap<TextureId, Texture>,
    samplers: HashMap<TextureOptions, wgpu::Sampler>,
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

        let texture_bind_group_layout =
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
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[&globals_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let depth_stencil = output_depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::Always,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                entry_point: Some("vs_main"),
                module: &shader_module,
                buffers: &[wgpu::VertexBufferLayout {
                    // 4x f32, 2x u32 -> 6 * 4 bytes
                    array_stride: 6 * 4,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    // 0: vec2 position
                    // 1: vec2 texture coordinates
                    // 2: uint color
                    // 3: uint layer_idx
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Uint32, 3 => Uint32],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default()
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
                compilation_options: wgpu::PipelineCompilationOptions::default()
            }),
            multiview: None,
            cache: None,
        });

        let vertex_buffer = SlicedBuffer::new(
            device,
            wgpu::BufferUsages::VERTEX,
            NonZeroU64::new(2048).expect("2048 is non-zero"),
            NonZeroU64::new(std::mem::size_of::<Vertex>() as u64)
                .expect("size of vertex is non-zero"),
        );
        let index_buffer = SlicedBuffer::new(
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
            texture_bind_group_layout,
            textures: HashMap::default(),
            samplers: HashMap::default(),
        }
    }

    pub fn render(
        &self,
        render_pass: &mut wgpu::RenderPass<'static>,
        paint_jobs: &[ClippedMesh],
        screen_size: PhysicalSize<u32>,
        pixels_per_point: f32,
    ) {
        profiling::scope!("render");

        let screen_rect = PhysicalRect::from_origin_and_size(Point::zero(), screen_size);
        let mut index_buffer_slices = self.index_buffer.slices.iter();
        let mut vertex_buffer_slices = self.vertex_buffer.slices.iter();

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

        for ClippedMesh { clip_rect, mesh } in paint_jobs {
            let phys_clip_rect = clip_rect.to_physical::<f32, _>(pixels_per_point).round();
            let scissor = phys_clip_rect
                // NOTE: can't cast to u32 directly because negative values cause a panic
                .cast::<i32>()
                .intersection(&screen_rect.to_i32())
                .map(|s| s.to_u32());

            let Some(scissor) = scissor else {
                index_buffer_slices.next().unwrap();
                vertex_buffer_slices.next().unwrap();
                continue;
            };

            render_pass.set_scissor_rect(
                scissor.min.x,
                scissor.min.y,
                scissor.width(),
                scissor.height(),
            );

            let index_buffer_slice = index_buffer_slices.next().unwrap();
            let vertex_buffer_slice = vertex_buffer_slices.next().unwrap();

            if let Some(Texture { bind_group, .. }) = self.textures.get(&mesh.texture_id) {
                render_pass.set_bind_group(1, bind_group, &[]);
                render_pass.set_index_buffer(
                    self.index_buffer
                        .buffer
                        .slice(index_buffer_slice.start as u64..index_buffer_slice.end as u64),
                    wgpu::IndexFormat::Uint32,
                );
                render_pass.set_vertex_buffer(
                    0,
                    self.vertex_buffer
                        .buffer
                        .slice(vertex_buffer_slice.start as u64..vertex_buffer_slice.end as u64),
                );
                render_pass.draw_indexed(0..mesh.indices.len() as u32, 0, 0..1);
            } else {
                log::warn!("Missing texture: {:?}", mesh.texture_id);
            }
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

            let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });

            let sampler = self
                .samplers
                .entry(image_delta.options)
                .or_insert_with(|| create_sampler(image_delta.options, device));

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label,
                layout: &self.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            });

            self.textures.insert(
                *id,
                Texture {
                    texture,
                    bind_group,
                },
            );
        }
    }

    pub fn free_textures(&mut self, textures_delta: &TexturesDelta) {
        profiling::scope!("free_textures");

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
        paint_jobs: &[ClippedMesh],
        screen_size: PhysicalSize<u32>,
        pixels_per_point: f32,
    ) {
        profiling::scope!("update_buffers");

        let uniform_buffer_content = Globals {
            screen_size: screen_size.to_logical(pixels_per_point),
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

        // count how many vertices & indices need to be rendered
        let (vertex_count, index_count) = {
            paint_jobs.iter().fold((0, 0), |acc, clipped_mesh| {
                (
                    acc.0 + clipped_mesh.mesh.vertices.len(),
                    acc.1 + clipped_mesh.mesh.indices.len(),
                )
            })
        };

        // update index and vertex buffers
        if index_count > 0 && vertex_count > 0 {
            self.index_buffer.slices.clear();
            self.vertex_buffer.slices.clear();

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

            let mut index_offset = 0;
            let mut vertex_offset = 0;
            for ClippedMesh { mesh, .. } in paint_jobs {
                {
                    let size = mesh.indices.len() * std::mem::size_of::<u32>();
                    let slice = index_offset..(index_offset + size);
                    staging_index_buffer[slice.clone()]
                        .copy_from_slice(bytemuck::cast_slice(&mesh.indices));
                    self.index_buffer.slices.push(slice);
                    index_offset += size;
                }
                {
                    let size = mesh.vertices.len() * std::mem::size_of::<Vertex>();
                    let slice = vertex_offset..(vertex_offset + size);
                    staging_vertex_buffer[slice.clone()]
                        .copy_from_slice(bytemuck::cast_slice(&mesh.vertices));
                    self.vertex_buffer.slices.push(slice);
                    vertex_offset += size;
                }
            }
        }
    }
}

struct SlicedBuffer {
    buffer: wgpu::Buffer,
    slices: Vec<Range<usize>>,
    size: wgpu::BufferSize,
    usage: wgpu::BufferUsages,
    stride: NonZeroU64,
}

impl SlicedBuffer {
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
            slices: Vec::with_capacity(64),
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
/// Doesn't upload any data for mip level > 0 if skip_mipmaps is true.
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
