use std::sync::Arc;
use wgpu::{BindGroup, Buffer, Device, Queue, RenderPipeline};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UnderlineInstance {
    pub pos:   [f32; 2],
    pub size:  [f32; 2],
    pub color: [f32; 4],
    pub style: u32,
    pub _pad:  [u32; 3],
}

impl UnderlineInstance {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UnderlineInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 0,  shader_location: 0 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 8,  shader_location: 1 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16, shader_location: 2 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint32,    offset: 32, shader_location: 3 },
            ],
        }
    }
}

pub struct UnderlineRenderer {
    pipeline:           RenderPipeline,
    screen_bind_group:  BindGroup,
    screen_buffer:      Buffer,
    instance_buffer:    Buffer,
    device:             Arc<Device>,
    last_count:         u32,
}

impl UnderlineRenderer {
    pub fn new(device: Arc<Device>, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("SYNAPSE_ Underline Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/underline.wgsl").into()),
        });

        let screen_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SYNAPSE_ Underline Screen BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding:    0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size:   None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label:                Some("SYNAPSE_ Underline PipelineLayout"),
            bind_group_layouts:   &[&screen_bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("SYNAPSE_ Underline Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module:              &shader,
                entry_point:         "vs_main",
                buffers:             &[UnderlineInstance::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:      &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format:     surface_format,
                    blend:      Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology:           wgpu::PrimitiveTopology::TriangleStrip,
                front_face:         wgpu::FrontFace::Ccw,
                cull_mode:          None,
                strip_index_format: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample:   wgpu::MultisampleState::default(),
            multiview:     None,
            cache:         None,
        });

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct ScreenUniform { screen_size: [f32; 2], _pad: [f32; 2] }

        let screen_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("SYNAPSE_ Underline Screen Buffer"),
            size:               std::mem::size_of::<ScreenUniform>() as u64,
            usage:              wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let screen_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("SYNAPSE_ Underline Screen BindGroup"),
            layout:  &screen_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding:  0,
                resource: screen_buffer.as_entire_binding(),
            }],
        });

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("SYNAPSE_ Underline Instance Buffer"),
            size:               256 * std::mem::size_of::<UnderlineInstance>() as u64,
            usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self { pipeline, screen_bind_group, screen_buffer, instance_buffer, device, last_count: 0 }
    }

    pub fn update_screen_size(&self, queue: &Queue, width: u32, height: u32) {
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct ScreenUniform { screen_size: [f32; 2], _pad: [f32; 2] }
        let u = ScreenUniform { screen_size: [width as f32, height as f32], _pad: [0.0; 2] };
        queue.write_buffer(&self.screen_buffer, 0, bytemuck::cast_slice(&[u]));
    }

    pub fn upload(&mut self, instances: &[UnderlineInstance], queue: &Queue) {
        let needed = std::mem::size_of_val(instances) as u64;
        if needed > self.instance_buffer.size() {
            let new_size = (needed * 2).next_power_of_two().max(256);
            self.instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label:              Some("SYNAPSE_ Underline Instance Buffer"),
                size:               new_size,
                usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !instances.is_empty() {
            queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(instances));
        }
        self.last_count = instances.len() as u32;
    }

    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if self.last_count == 0 { return; }
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.screen_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        render_pass.draw(0..4, 0..self.last_count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_size_is_48_bytes() {
        assert_eq!(std::mem::size_of::<UnderlineInstance>(), 48);
    }
}
