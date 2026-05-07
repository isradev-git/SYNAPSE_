use std::collections::HashMap;
use std::sync::Arc;
use wgpu::{Device, Instance, Queue, Surface, SurfaceConfiguration};
use winit::window::Window;

use crate::atlas::TextureAtlas;
use crate::cell::{CellInstance, CellRenderer};
use crate::text::TextShaping;
use crate::ui::{UIRect, UIRenderer};

pub struct Renderer {
    surface: Surface<'static>,
    device: Device,
    queue: Queue,
    config: SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    atlas: TextureAtlas,
    cell_renderer: CellRenderer,
    ui_renderer: UIRenderer,
    text: TextShaping,
}

impl Renderer {
    pub fn new(window: Arc<Window>) -> Self {
        let instance = Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).expect("Failed to create surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find suitable GPU adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Luna GPU Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .expect("Failed to create GPU device");

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        let atlas = TextureAtlas::new(&device);
        let cell_renderer = CellRenderer::new(&device, &atlas.bind_group_layout, config.format);
        let ui_renderer = UIRenderer::new(&device, config.format);
        let text = TextShaping::new();

        Self {
            surface,
            device,
            queue,
            config,
            size,
            atlas,
            cell_renderer,
            ui_renderer,
            text,
        }
    }

    pub fn size(&self) -> winit::dpi::PhysicalSize<u32> {
        self.size
    }

    pub fn cell_metrics(&mut self, font_size: f32) -> (f32, f32) {
        self.text.cell_metrics(font_size)
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn draw_text(
        &mut self,
        text_str: &str,
        x: f32,
        y: f32,
        font_size: f32,
        fg: [f32; 4],
        bg: [f32; 4],
    ) {
        let mut instances: Vec<CellInstance> = Vec::new();
        let mut x_offset = x;

        for c in text_str.chars() {
            if let Some((swash_image, cache_key)) = self.text.rasterize_glyph(c, font_size) {
                let bitmap_width = swash_image.placement.width as u32;
                let bitmap_height = swash_image.placement.height as u32;

                if bitmap_width == 0 || bitmap_height == 0 {
                    x_offset += swash_image.placement.left as f32 + font_size * 0.6;
                    continue;
                }

                let uv = self
                    .atlas
                    .get_or_insert(cache_key, bitmap_width, bitmap_height);

                if let Some(uv) = uv {
                    let pixels: Vec<u8> = match &swash_image.content {
                        cosmic_text::SwashContent::Mask => {
                            let mut rgba = vec![0u8; (bitmap_width * bitmap_height * 4) as usize];
                            for i in 0..swash_image.data.len() {
                                rgba[i * 4] = 255;
                                rgba[i * 4 + 1] = 255;
                                rgba[i * 4 + 2] = 255;
                                rgba[i * 4 + 3] = swash_image.data[i];
                            }
                            rgba
                        }
                        cosmic_text::SwashContent::SubpixelMask => {
                            swash_image.data.clone()
                        }
                        cosmic_text::SwashContent::Color => {
                            swash_image.data.clone()
                        }
                    };

                    self.atlas
                        .upload_glyph(&self.queue, uv, &pixels, bitmap_width, bitmap_height);

                    let glyph_x = x_offset + swash_image.placement.left as f32;
                    let glyph_y = y - swash_image.placement.top as f32;

                    instances.push(CellInstance {
                        cell_pos: [glyph_x, glyph_y],
                        cell_size: [bitmap_width as f32, bitmap_height as f32],
                        uv_rect: [uv.u0, uv.v0, uv.u1, uv.v1],
                        fg_color: fg,
                        bg_color: bg,
                    });
                }

                x_offset += swash_image.placement.left as f32 + font_size * 0.6;
            } else {
                x_offset += font_size * 0.6;
            }
        }

        self.render_instances(&instances, &[]);
    }

    pub fn draw_cells(
        &mut self,
        cells: &[(char, f32, f32, f32, [f32; 4], [f32; 4])],
    ) {
        let mut instances: Vec<CellInstance> = Vec::with_capacity(cells.len());
        let mut seen: HashMap<(char, u32), (cosmic_text::SwashImage, cosmic_text::CacheKey)> =
            HashMap::new();

        for &(c, x, y, font_size, fg, bg) in cells {
            if c == ' ' {
                continue;
            }

            let key = (c, font_size.to_bits());

            let (swash_image, cache_key) = if let Some(cached) = seen.get(&key) {
                (cached.0.clone(), cached.1)
            } else {
                match self.text.rasterize_glyph(c, font_size) {
                    Some(result) => {
                        seen.insert(key, (result.0.clone(), result.1));
                        result
                    }
                    None => continue,
                }
            };

            let bitmap_width = swash_image.placement.width as u32;
            let bitmap_height = swash_image.placement.height as u32;

            if bitmap_width == 0 || bitmap_height == 0 {
                continue;
            }

            let uv = self
                .atlas
                .get_or_insert(cache_key, bitmap_width, bitmap_height);

            if let Some(uv) = uv {
                let pixels: Vec<u8> = match &swash_image.content {
                    cosmic_text::SwashContent::Mask => {
                        let mut rgba = vec![0u8; (bitmap_width * bitmap_height * 4) as usize];
                        for i in 0..swash_image.data.len() {
                            rgba[i * 4] = 255;
                            rgba[i * 4 + 1] = 255;
                            rgba[i * 4 + 2] = 255;
                            rgba[i * 4 + 3] = swash_image.data[i];
                        }
                        rgba
                    }
                    cosmic_text::SwashContent::SubpixelMask => {
                        swash_image.data.clone()
                    }
                    cosmic_text::SwashContent::Color => {
                        swash_image.data.clone()
                    }
                };

                self.atlas
                    .upload_glyph(&self.queue, uv, &pixels, bitmap_width, bitmap_height);

                let glyph_x = x + swash_image.placement.left as f32;
                let glyph_y = y - swash_image.placement.top as f32;

                instances.push(CellInstance {
                    cell_pos: [glyph_x, glyph_y],
                    cell_size: [bitmap_width as f32, bitmap_height as f32],
                    uv_rect: [uv.u0, uv.v0, uv.u1, uv.v1],
                    fg_color: fg,
                    bg_color: bg,
                });
            }
        }

        self.render_instances(&instances, &[]);
    }

    pub fn draw_ui_rects(&mut self, rects: &[UIRect]) {
        self.render_instances(&[], rects);
    }

    pub fn draw_frame(
        &mut self,
        cells: &[(char, f32, f32, f32, [f32; 4], [f32; 4])],
        ui_rects: &[UIRect],
    ) {
        let mut instances: Vec<CellInstance> = Vec::with_capacity(cells.len());
        let mut seen: HashMap<(char, u32), (cosmic_text::SwashImage, cosmic_text::CacheKey)> =
            HashMap::new();

        for &(c, x, y, font_size, fg, bg) in cells {
            if c == ' ' {
                continue;
            }

            let key = (c, font_size.to_bits());

            let (swash_image, cache_key) = if let Some(cached) = seen.get(&key) {
                (cached.0.clone(), cached.1)
            } else {
                match self.text.rasterize_glyph(c, font_size) {
                    Some(result) => {
                        seen.insert(key, (result.0.clone(), result.1));
                        result
                    }
                    None => continue,
                }
            };

            let bitmap_width = swash_image.placement.width as u32;
            let bitmap_height = swash_image.placement.height as u32;

            if bitmap_width == 0 || bitmap_height == 0 {
                continue;
            }

            let uv = self
                .atlas
                .get_or_insert(cache_key, bitmap_width, bitmap_height);

            if let Some(uv) = uv {
                let pixels: Vec<u8> = match &swash_image.content {
                    cosmic_text::SwashContent::Mask => {
                        let mut rgba = vec![0u8; (bitmap_width * bitmap_height * 4) as usize];
                        for i in 0..swash_image.data.len() {
                            rgba[i * 4] = 255;
                            rgba[i * 4 + 1] = 255;
                            rgba[i * 4 + 2] = 255;
                            rgba[i * 4 + 3] = swash_image.data[i];
                        }
                        rgba
                    }
                    cosmic_text::SwashContent::SubpixelMask => {
                        swash_image.data.clone()
                    }
                    cosmic_text::SwashContent::Color => {
                        swash_image.data.clone()
                    }
                };

                self.atlas
                    .upload_glyph(&self.queue, uv, &pixels, bitmap_width, bitmap_height);

                let glyph_x = x + swash_image.placement.left as f32;
                let glyph_y = y - swash_image.placement.top as f32;

                instances.push(CellInstance {
                    cell_pos: [glyph_x, glyph_y],
                    cell_size: [bitmap_width as f32, bitmap_height as f32],
                    uv_rect: [uv.u0, uv.v0, uv.u1, uv.v1],
                    fg_color: fg,
                    bg_color: bg,
                });
            }
        }

        self.render_instances(&instances, ui_rects);
    }

    fn render_instances(&mut self, cell_instances: &[CellInstance], ui_rects: &[UIRect]) {
        self.cell_renderer
            .update_screen_size(&self.queue, self.size.width, self.size.height);
        self.ui_renderer
            .update_screen_size(&self.queue, self.size.width, self.size.height);

        let output = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Err(e) => {
                eprintln!("Surface error: {:?}", e);
                return;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Luna Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Luna Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 33.0 / 255.0,
                            g: 11.0 / 255.0,
                            b: 75.0 / 255.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.cell_renderer.draw(
                &mut render_pass,
                &self.atlas.bind_group,
                cell_instances,
                &self.queue,
            );

            if !ui_rects.is_empty() {
                self.ui_renderer.draw(
                    &mut render_pass,
                    ui_rects,
                    &self.queue,
                );
            }
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.cell_renderer
            .update_screen_size(&self.queue, self.size.width, self.size.height);

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Luna Render Encoder"),
            });

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Luna Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 33.0 / 255.0,
                            g: 11.0 / 255.0,
                            b: 75.0 / 255.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();

        Ok(())
    }
}
