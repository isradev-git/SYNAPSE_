use std::sync::Arc;
use wgpu::{BindGroup, Buffer, Device, Queue, RenderPipeline, Texture, TextureView};

pub const EFFECT_SCANLINES: u32 = 1 << 0;
pub const EFFECT_BLOOM:     u32 = 1 << 1;
pub const EFFECT_CHROMA:    u32 = 1 << 2;
pub const EFFECT_GLITCH:    u32 = 1 << 3;
pub const EFFECT_MATRIX_BG: u32 = 1 << 4;
pub const EFFECT_HEX_GRID:  u32 = 1 << 5;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PostProcUniform {
    pub screen_size:        [f32; 2],  // offset  0
    pub time:               f32,        // offset  8
    pub effects_mask:       u32,        // offset 12
    pub scanline_intensity: f32,        // offset 16
    pub scanline_freq:      f32,        // offset 20
    pub bloom_threshold:    f32,        // offset 24
    pub bloom_sigma:        f32,        // offset 28
    pub bloom_tint:         [f32; 4],   // offset 32
    pub chroma_strength:    f32,        // offset 48
    pub glitch_intensity:   f32,        // offset 52
    pub matrix_density:     f32,        // offset 56
    pub _pad:               f32,        // offset 60
    pub matrix_color:       [f32; 4],   // offset 64
    // total: 80 bytes
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BloomUniform {
    pub screen_size: [f32; 2],
    pub threshold:   f32,
    pub sigma:       f32,
}

pub struct PostProcRenderer {
    device: Arc<Device>,
    format: wgpu::TextureFormat,
    offscreen_tex:  Texture,
    offscreen_view: TextureView,
    bloom_h_tex:  Texture,
    bloom_h_view: TextureView,
    bloom_h_pipeline:    RenderPipeline,
    bloom_h_scene_bg:    BindGroup,
    bloom_h_uniform_buf: Buffer,
    bloom_h_uniform_bg:  BindGroup,
    main_pipeline:    RenderPipeline,
    main_scene_bg:    BindGroup,
    main_uniform_buf: Buffer,
    main_uniform_bg:  BindGroup,
}

impl PostProcRenderer {
    pub fn new(
        device: Arc<Device>,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let offscreen_tex = Self::make_tex(&device, format, width, height, "SYNAPSE_ Offscreen");
        let offscreen_view = offscreen_tex.create_view(&Default::default());

        let bloom_h_tex = Self::make_tex(&device, format, width, height, "SYNAPSE_ BloomH");
        let bloom_h_view = bloom_h_tex.create_view(&Default::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label:          Some("SYNAPSE_ PostProc Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter:     wgpu::FilterMode::Linear,
            min_filter:     wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // ── Bloom H pass ──────────────────────────────────────────────────────

        let bloom_h_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("SYNAPSE_ BloomH Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/bloom_h.wgsl").into()),
        });

        let bloom_h_tex_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:   Some("SYNAPSE_ BloomH Tex BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding:    0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type:    wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled:   false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding:    1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bloom_h_uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:   Some("SYNAPSE_ BloomH Uniform BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding:    0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty:                 wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size:   None,
                },
                count: None,
            }],
        });

        let bloom_h_uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("SYNAPSE_ BloomH Uniform"),
            size:               std::mem::size_of::<BloomUniform>() as u64,
            usage:              wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bloom_h_scene_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("SYNAPSE_ BloomH Scene BG"),
            layout:  &bloom_h_tex_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding:  0,
                    resource: wgpu::BindingResource::TextureView(&offscreen_view),
                },
                wgpu::BindGroupEntry {
                    binding:  1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let bloom_h_uniform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("SYNAPSE_ BloomH Uniform BG"),
            layout:  &bloom_h_uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding:  0,
                resource: bloom_h_uniform_buf.as_entire_binding(),
            }],
        });

        let bloom_h_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label:                Some("SYNAPSE_ BloomH Pipeline Layout"),
            bind_group_layouts:   &[&bloom_h_tex_bgl, &bloom_h_uniform_bgl],
            push_constant_ranges: &[],
        });

        let bloom_h_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("SYNAPSE_ BloomH Pipeline"),
            layout: Some(&bloom_h_pipeline_layout),
            vertex: wgpu::VertexState {
                module:              &bloom_h_shader,
                entry_point:         "vs_main",
                buffers:             &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:              &bloom_h_shader,
                entry_point:         "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend:      Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive:     wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample:   wgpu::MultisampleState::default(),
            multiview:     None,
            cache:         None,
        });

        // ── Main composite pass ───────────────────────────────────────────────

        let main_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("SYNAPSE_ PostProc Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/postproc.wgsl").into()),
        });

        let main_tex_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:   Some("SYNAPSE_ PostProc Tex BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding:    0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type:    wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled:   false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding:    1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding:    2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type:    wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled:   false,
                    },
                    count: None,
                },
            ],
        });

        let main_uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:   Some("SYNAPSE_ PostProc Uniform BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding:    0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty:                 wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size:   None,
                },
                count: None,
            }],
        });

        let main_uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("SYNAPSE_ PostProc Uniform"),
            size:               std::mem::size_of::<PostProcUniform>() as u64,
            usage:              wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let main_scene_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("SYNAPSE_ PostProc Scene BG"),
            layout:  &main_tex_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding:  0,
                    resource: wgpu::BindingResource::TextureView(&offscreen_view),
                },
                wgpu::BindGroupEntry {
                    binding:  1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding:  2,
                    resource: wgpu::BindingResource::TextureView(&bloom_h_view),
                },
            ],
        });

        let main_uniform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("SYNAPSE_ PostProc Uniform BG"),
            layout:  &main_uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding:  0,
                resource: main_uniform_buf.as_entire_binding(),
            }],
        });

        let main_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label:                Some("SYNAPSE_ PostProc Pipeline Layout"),
            bind_group_layouts:   &[&main_tex_bgl, &main_uniform_bgl],
            push_constant_ranges: &[],
        });

        let main_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("SYNAPSE_ PostProc Pipeline"),
            layout: Some(&main_pipeline_layout),
            vertex: wgpu::VertexState {
                module:              &main_shader,
                entry_point:         "vs_main",
                buffers:             &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:              &main_shader,
                entry_point:         "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend:      Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive:     wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample:   wgpu::MultisampleState::default(),
            multiview:     None,
            cache:         None,
        });

        Self {
            device,
            format,
            offscreen_tex,
            offscreen_view,
            bloom_h_tex,
            bloom_h_view,
            bloom_h_pipeline,
            bloom_h_scene_bg,
            bloom_h_uniform_buf,
            bloom_h_uniform_bg,
            main_pipeline,
            main_scene_bg,
            main_uniform_buf,
            main_uniform_bg,
        }
    }

    fn make_tex(device: &Device, format: wgpu::TextureFormat, w: u32, h: u32, label: &str) -> Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label:                 Some(label),
            size:                  wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count:       1,
            sample_count:          1,
            dimension:             wgpu::TextureDimension::D2,
            format,
            usage:                 wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats:          &[],
        })
    }

    pub fn resize(&mut self, width: u32, height: u32, queue: &Queue) {
        self.offscreen_tex  = Self::make_tex(&self.device, self.format, width, height, "SYNAPSE_ Offscreen");
        self.offscreen_view = self.offscreen_tex.create_view(&Default::default());
        self.bloom_h_tex    = Self::make_tex(&self.device, self.format, width, height, "SYNAPSE_ BloomH");
        self.bloom_h_view   = self.bloom_h_tex.create_view(&Default::default());

        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter:     wgpu::FilterMode::Linear,
            min_filter:     wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bloom_h_tex_bgl = self.bloom_h_pipeline.get_bind_group_layout(0);
        self.bloom_h_scene_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("SYNAPSE_ BloomH Scene BG"),
            layout:  &bloom_h_tex_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding:  0,
                    resource: wgpu::BindingResource::TextureView(&self.offscreen_view),
                },
                wgpu::BindGroupEntry {
                    binding:  1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let main_tex_bgl = self.main_pipeline.get_bind_group_layout(0);
        self.main_scene_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("SYNAPSE_ PostProc Scene BG"),
            layout:  &main_tex_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding:  0,
                    resource: wgpu::BindingResource::TextureView(&self.offscreen_view),
                },
                wgpu::BindGroupEntry {
                    binding:  1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding:  2,
                    resource: wgpu::BindingResource::TextureView(&self.bloom_h_view),
                },
            ],
        });

        let _ = queue;
    }

    pub fn offscreen_view(&self) -> &TextureView {
        &self.offscreen_view
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &TextureView,
        queue: &Queue,
        uniform: &PostProcUniform,
    ) {
        queue.write_buffer(&self.main_uniform_buf, 0, bytemuck::bytes_of(uniform));

        let bloom_on = uniform.effects_mask & EFFECT_BLOOM != 0;
        if bloom_on {
            let bloom_uniform = BloomUniform {
                screen_size: uniform.screen_size,
                threshold:   uniform.bloom_threshold,
                sigma:       uniform.bloom_sigma,
            };
            queue.write_buffer(&self.bloom_h_uniform_buf, 0, bytemuck::bytes_of(&bloom_uniform));

            let mut bloom_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SYNAPSE_ Bloom H Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view:           &self.bloom_h_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load:  wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes:         None,
                occlusion_query_set:      None,
            });
            bloom_pass.set_pipeline(&self.bloom_h_pipeline);
            bloom_pass.set_bind_group(0, &self.bloom_h_scene_bg, &[]);
            bloom_pass.set_bind_group(1, &self.bloom_h_uniform_bg, &[]);
            bloom_pass.draw(0..3, 0..1);
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("SYNAPSE_ PostProc Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view:           surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load:  wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes:         None,
            occlusion_query_set:      None,
        });
        pass.set_pipeline(&self.main_pipeline);
        pass.set_bind_group(0, &self.main_scene_bg, &[]);
        pass.set_bind_group(1, &self.main_uniform_bg, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_postproc_uniform_size() {
        assert_eq!(std::mem::size_of::<PostProcUniform>(), 80);
    }

    #[test]
    fn test_bloom_uniform_size() {
        assert_eq!(std::mem::size_of::<BloomUniform>(), 16);
    }

    #[test]
    fn test_effect_bitmasks_unique() {
        let masks = [
            EFFECT_SCANLINES, EFFECT_BLOOM, EFFECT_CHROMA,
            EFFECT_GLITCH, EFFECT_MATRIX_BG, EFFECT_HEX_GRID,
        ];
        for (i, &a) in masks.iter().enumerate() {
            for (j, &b) in masks.iter().enumerate() {
                if i != j {
                    assert_eq!(a & b, 0, "bitmask collision at indices {i} and {j}");
                }
            }
        }
    }
}
