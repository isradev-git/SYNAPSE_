use std::collections::HashMap;
use std::sync::Arc;
use wgpu::{Device, Instance, Queue, Surface, SurfaceConfiguration};
use winit::window::Window;

use crate::atlas::TextureAtlas;
use crate::cell::{CellInstance, CellRenderer};
use crate::text::{ShapedGlyph, TextShaping};
use crate::ui::{UIRect, UIRenderer};

pub struct Renderer {
    surface: Surface<'static>,
    device: Arc<Device>,
    queue: Queue,
    config: SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    atlas: TextureAtlas,
    cell_renderer: CellRenderer,
    ui_renderer: UIRenderer,
    text: TextShaping,
    clear_color: wgpu::Color,
    font_ligatures: bool,
    cell_w: f32,
}

impl Renderer {
    pub fn new(window: Arc<Window>) -> Result<Self, String> {
        let instance = Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| format!("Failed to create GPU surface: {}", e))?;

        let adapter = match pollster::block_on(instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        )) {
            Some(adapter) => adapter,
            None => {
                return Err(
                    "No compatible GPU found. Luna requires DX12 (Windows), Metal (macOS), or Vulkan (Linux).".into(),
                )
            }
        };

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Luna GPU Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .map_err(|e| format!("Failed to create GPU device: {}", e))?;

        let device = Arc::new(device);

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
        let cell_renderer =
            CellRenderer::new(Arc::clone(&device), &atlas.bind_group_layout, config.format);
        let ui_renderer = UIRenderer::new(Arc::clone(&device), config.format);
        let text = TextShaping::new();

        Ok(Self {
            surface,
            device,
            queue,
            clear_color: wgpu::Color {
                r: 33.0 / 255.0,
                g: 11.0 / 255.0,
                b: 75.0 / 255.0,
                a: 1.0,
            },
            config,
            size,
            atlas,
            cell_renderer,
            ui_renderer,
            text,
            font_ligatures: false,
            cell_w: 0.0,
        })
    }

    pub fn set_font_ligatures(&mut self, enabled: bool) {
        self.font_ligatures = enabled;
    }

    pub fn size(&self) -> winit::dpi::PhysicalSize<u32> {
        self.size
    }

    pub fn set_clear_color(&mut self, color: [f32; 4]) {
        self.clear_color = wgpu::Color {
            r: color[0] as f64,
            g: color[1] as f64,
            b: color[2] as f64,
            a: color[3] as f64,
        };
    }

    pub fn cell_metrics(&mut self, font_size: f32) -> (f32, f32) {
        let metrics = self.text.cell_metrics(font_size);
        self.cell_w = metrics.0;
        metrics
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
                let advance = swash_image.placement.left as f32 + font_size * 0.6;
                self.push_glyph_instance(
                    &mut instances,
                    &swash_image,
                    cache_key,
                    x_offset,
                    y,
                    fg,
                    bg,
                );
                x_offset += advance;
            } else {
                x_offset += font_size * 0.6;
            }
        }

        self.render_instances(&instances, &[]);
    }

    #[allow(clippy::type_complexity)]
    pub fn draw_cells(&mut self, cells: &[(char, f32, f32, f32, [f32; 4], [f32; 4])]) {
        let instances = self.build_simple_instances(cells);
        self.render_instances(&instances, &[]);
    }

    pub fn draw_ui_rects(&mut self, rects: &[UIRect]) {
        self.render_instances(&[], rects);
    }

    fn swash_to_rgba(image: &cosmic_text::SwashImage) -> Vec<u8> {
        match &image.content {
            cosmic_text::SwashContent::Mask => {
                let mut rgba =
                    vec![0u8; (image.placement.width * image.placement.height * 4) as usize];
                for (i, &alpha) in image.data.iter().enumerate() {
                    rgba[i * 4] = 255;
                    rgba[i * 4 + 1] = 255;
                    rgba[i * 4 + 2] = 255;
                    rgba[i * 4 + 3] = alpha;
                }
                rgba
            }
            _ => image.data.clone(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn push_glyph_instance(
        &mut self,
        instances: &mut Vec<CellInstance>,
        image: &cosmic_text::SwashImage,
        cache_key: cosmic_text::CacheKey,
        cell_x: f32,
        cell_y: f32,
        fg: [f32; 4],
        bg: [f32; 4],
    ) {
        let bw = image.placement.width;
        let bh = image.placement.height;
        if bw == 0 || bh == 0 {
            return;
        }
        if let Some(uv) = self.atlas.get_or_insert(cache_key, bw, bh) {
            let pixels = Self::swash_to_rgba(image);
            self.atlas.upload_glyph(&self.queue, uv, &pixels, bw, bh);
            instances.push(CellInstance {
                cell_pos: [
                    cell_x + image.placement.left as f32,
                    cell_y - image.placement.top as f32,
                ],
                cell_size: [bw as f32, bh as f32],
                uv_rect: [uv.u0, uv.v0, uv.u1, uv.v1],
                fg_color: fg,
                bg_color: bg,
            });
        }
    }

    /// Build instances with ligature-aware text shaping.
    /// Groups consecutive same-style non-space cells into runs, shapes each run as a
    /// unit so the font's GSUB rules can apply ligature substitutions, then maps
    /// the resulting glyphs back to cell positions.
    #[allow(clippy::type_complexity)]
    fn build_ligature_instances(
        &mut self,
        cells: &[(char, f32, f32, f32, [f32; 4], [f32; 4])],
    ) -> Vec<CellInstance> {
        let cell_w = self.cell_w;
        let mut instances = Vec::with_capacity(cells.len());

        let non_space: Vec<_> = cells.iter().filter(|&&(c, ..)| c != ' ').copied().collect();

        let mut i = 0;
        while i < non_space.len() {
            let (_, x0, y0, fs0, fg0, bg0) = non_space[i];

            // Collect a run: consecutive cells, same row + style, x advances by cell_w.
            let mut run: Vec<(char, f32)> = vec![(non_space[i].0, x0)];
            let mut j = i + 1;
            while j < non_space.len() {
                let (c, x, y, fs, fg, bg) = non_space[j];
                let prev_x = run.last().unwrap().1;
                let contiguous = (y - y0).abs() < 0.5
                    && (fs - fs0).abs() < 0.1
                    && fg == fg0
                    && bg == bg0
                    && (x - prev_x - cell_w).abs() < 1.0;
                if !contiguous {
                    break;
                }
                run.push((c, x));
                j += 1;
            }

            if run.len() == 1 {
                let (c, x) = run[0];
                if let Some((image, cache_key)) = self.text.rasterize_glyph(c, fs0) {
                    self.push_glyph_instance(&mut instances, &image, cache_key, x, y0, fg0, bg0);
                }
            } else {
                let run_text: String = run.iter().map(|&(c, _)| c).collect();
                // shape_run takes &mut self, so collect first to avoid borrow conflict
                let shaped: Vec<ShapedGlyph> = self.text.shape_run(&run_text, fs0);
                for sg in shaped {
                    // map byte offset → char index → cell x position
                    let char_idx = run_text[..sg.src_start].chars().count();
                    if char_idx >= run.len() {
                        continue;
                    }
                    let cell_x = run[char_idx].1;
                    self.push_glyph_instance(
                        &mut instances,
                        &sg.image,
                        sg.cache_key,
                        cell_x,
                        y0,
                        fg0,
                        bg0,
                    );
                }
            }

            i = j;
        }

        instances
    }

    #[allow(clippy::type_complexity)]
    pub fn draw_frame(
        &mut self,
        cells: &[(char, f32, f32, f32, [f32; 4], [f32; 4])],
        ui_rects: &[UIRect],
    ) {
        self.atlas.begin_frame();

        let instances = if self.font_ligatures && self.cell_w > 0.0 {
            self.build_ligature_instances(cells)
        } else {
            self.build_simple_instances(cells)
        };

        self.render_instances(&instances, ui_rects);
    }

    #[allow(clippy::type_complexity)]
    fn build_simple_instances(
        &mut self,
        cells: &[(char, f32, f32, f32, [f32; 4], [f32; 4])],
    ) -> Vec<CellInstance> {
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

            self.push_glyph_instance(&mut instances, &swash_image, cache_key, x, y, fg, bg);
        }

        instances
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
                        load: wgpu::LoadOp::Clear(self.clear_color),
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
                self.ui_renderer
                    .draw(&mut render_pass, ui_rects, &self.queue);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();
    }
}
