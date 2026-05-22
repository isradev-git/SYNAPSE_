use std::collections::HashMap;
use std::sync::Arc;
use wgpu::{BindGroup, BindGroupLayout, Buffer, Device, Queue, RenderPipeline, Sampler, Texture};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ImageInstance {
    pub pos: [f32; 2],
    pub size: [f32; 2],
}

impl ImageInstance {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ImageInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 1,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ScreenUniform {
    screen_size: [f32; 2],
    _padding: [f32; 2],
}

pub struct GpuTexture {
    pub texture: Texture,
    pub bind_group: BindGroup,
    width: u32,
    height: u32,
}

pub struct ImageRenderer {
    pipeline: RenderPipeline,
    screen_bind_group: BindGroup,
    screen_buffer: Buffer,
    instance_buffer: Buffer,
    image_sampler: Sampler,
    pub texture_bind_group_layout: BindGroupLayout,
    textures: HashMap<u32, GpuTexture>,
    device: Arc<Device>,
}

impl ImageRenderer {
    pub fn new(device: Arc<Device>, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SYNAPSE_ Image Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/image.wgsl").into()),
        });

        let screen_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("SYNAPSE_ Image Screen BindGroupLayout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("SYNAPSE_ Image Texture BindGroupLayout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
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
            label: Some("SYNAPSE_ Image PipelineLayout"),
            bind_group_layouts: &[&screen_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SYNAPSE_ Image Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[ImageInstance::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let screen_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SYNAPSE_ Image Screen Uniform Buffer"),
            size: std::mem::size_of::<ScreenUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let screen_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SYNAPSE_ Image Screen BindGroup"),
            layout: &screen_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: screen_buffer.as_entire_binding(),
            }],
        });

        let image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SYNAPSE_ Image Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SYNAPSE_ Image Instance Buffer"),
            size: 256,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            screen_bind_group,
            screen_buffer,
            instance_buffer,
            image_sampler,
            texture_bind_group_layout,
            textures: HashMap::new(),
            device: device.clone(),
        }
    }

    pub fn update_screen_size(&self, queue: &Queue, width: u32, height: u32) {
        let uniform = ScreenUniform {
            screen_size: [width as f32, height as f32],
            _padding: [0.0; 2],
        };
        queue.write_buffer(&self.screen_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }

    pub fn has_image(&self, id: u32) -> bool {
        self.textures.contains_key(&id)
    }

    pub fn upload_image(&mut self, id: u32, rgba: &[u8], width: u32, height: u32, queue: &Queue) {
        use wgpu::util::DeviceExt;

        let texture = self.device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some(&format!("SYNAPSE_ Image Texture {}", id)),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            rgba,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("SYNAPSE_ Image BindGroup {}", id)),
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.image_sampler),
                },
            ],
        });

        self.textures.insert(
            id,
            GpuTexture {
                texture,
                bind_group,
                width,
                height,
            },
        );
    }

    pub fn remove_image(&mut self, id: u32) {
        self.textures.remove(&id);
    }

    pub fn image_dimensions(&self, id: u32) -> Option<(u32, u32)> {
        self.textures.get(&id).map(|t| (t.width, t.height))
    }

    pub fn draw_images<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        instances: &[ImageInstance],
        texture_ids: &[u32],
        clip_rects: &[[u32; 4]],
        queue: &Queue,
    ) {
        if instances.is_empty() || texture_ids.is_empty() {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.screen_bind_group, &[]);

        for (i, instance) in instances.iter().enumerate() {
            let tex_id = texture_ids[i];
            let gpu_tex = match self.textures.get(&tex_id) {
                Some(t) => t,
                None => continue,
            };

            queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&[*instance]));

            render_pass.set_bind_group(1, &gpu_tex.bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));

            if i < clip_rects.len() {
                let [cx, cy, cw, ch] = clip_rects[i];
                if cw > 0 && ch > 0 {
                    render_pass.set_scissor_rect(cx, cy, cw, ch);
                }
            }

            render_pass.draw(0..4, 0..1);
        }
    }
}
