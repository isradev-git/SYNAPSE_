use std::sync::Arc;
use wgpu::{Device, Instance, Queue, Surface, SurfaceConfiguration};
use winit::window::Window;

use crate::atlas::TextureAtlas;
use crate::cell::{CellInstance, CellRenderer};
use crate::text::TextShaping;
use crate::ui::{UIRect, UIRenderer};

pub struct Renderer {
    surface: Surface<'static>,
    device: Arc<Device>,
    queue: Queue,
    config: SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    atlas: TextureAtlas,
    cell_renderer: CellRenderer,
    ui_renderer_bg: UIRenderer,
    ui_renderer: UIRenderer,
    text: TextShaping,
    clear_color: wgpu::Color,
    cell_w: f32,
    cell_h: f32,
    cached_instances: Vec<CellInstance>,
}

impl Renderer {
    pub fn new(window: Arc<Window>, font_family: &str) -> Result<Self, String> {
        let instance_desc = wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        };

        let instance = Instance::new(instance_desc);

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| format!("Failed to create GPU surface: {}", e))?;

        let adapter =
            match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })) {
                Some(adapter) => adapter,
                None => match pollster::block_on(instance.request_adapter(
                    &wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::LowPower,
                        compatible_surface: Some(&surface),
                        force_fallback_adapter: true,
                    },
                )) {
                    Some(fallback) => {
                        tracing::warn!(
                            "Using software/fallback GPU adapter. Performance may be reduced."
                        );
                        fallback
                    }
                    None => {
                        let msg = "No compatible GPU found.\n\
                         Luna requires Vulkan (Linux), or Metal (macOS).\n\
                         On VMs, enable 3D acceleration or GPU passthrough."
                            .to_string();
                        return Err(msg);
                    }
                },
            };

        let adapter_info = adapter.get_info();
        tracing::info!(
            "GPU adapter: {} ({:?} backend), driver: {}",
            adapter_info.name,
            adapter_info.backend,
            adapter_info.driver_info
        );

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("SYNAPSE_ GPU Device"),
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
        // We store colors in sRGB-encoded floats (e.g. 0x11/255.0 = 0.0667) and
        // write them directly from the shader. If the surface is sRGB, wgpu
        // does an automatic linear→sRGB conversion at present time, which
        // brightens everything because it assumes the shader output was linear.
        // Prefer a non-sRGB (UNORM) format so what we write is what gets shown.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
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
        let ui_renderer_bg = UIRenderer::new(Arc::clone(&device), config.format);
        let ui_renderer = UIRenderer::new(Arc::clone(&device), config.format);
        let text = TextShaping::with_family(font_family);

        Ok(Self {
            surface,
            device,
            queue,
            clear_color: wgpu::Color {
                r: 17.0 / 255.0,
                g: 19.0 / 255.0,
                b: 26.0 / 255.0,
                a: 1.0,
            },
            config,
            size,
            atlas,
            cell_renderer,
            ui_renderer_bg,
            ui_renderer,
            text,
            cell_w: 0.0,
            cell_h: 0.0,
            cached_instances: Vec::new(),
        })
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
        self.cell_h = metrics.1;
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
        _bg: [f32; 4],
    ) {
        let mut instances: Vec<CellInstance> = Vec::new();
        let mut x_offset = x;

        let cell_w = self.cell_w;
        for c in text_str.chars() {
            if c == ' ' {
                x_offset += cell_w;
                continue;
            }
            let key = crate::text::GlyphKey::new(c, font_size, false, false);
            let bitmap = self.text.rasterize(key);
            self.push_glyph_instance(&mut instances, &bitmap, key, x_offset, y, font_size, fg);
            x_offset += cell_w;
        }

        self.cell_renderer.upload(&instances, &self.queue);
        self.ui_renderer_bg.upload(&[], &self.queue);
        self.ui_renderer.upload(&[], &self.queue);
        self.render_instances();
    }

    #[allow(clippy::type_complexity)]
    pub fn draw_cells(&mut self, cells: &[(char, f32, f32, f32, [f32; 4], [f32; 4])]) {
        let instances = self.build_simple_instances(cells);
        self.cell_renderer.upload(&instances, &self.queue);
        self.ui_renderer_bg.upload(&[], &self.queue);
        self.ui_renderer.upload(&[], &self.queue);
        self.render_instances();
    }

    pub fn draw_ui_rects(&mut self, rects: &[UIRect]) {
        self.cell_renderer.upload(&[], &self.queue);
        self.ui_renderer_bg.upload(&[], &self.queue);
        self.ui_renderer.upload(rects, &self.queue);
        self.render_instances();
    }

    fn gray_to_rgba(gray: &[u8]) -> Vec<u8> {
        let mut rgba = vec![0u8; gray.len() * 4];
        for (i, &alpha) in gray.iter().enumerate() {
            rgba[i * 4] = 255;
            rgba[i * 4 + 1] = 255;
            rgba[i * 4 + 2] = 255;
            rgba[i * 4 + 3] = alpha;
        }
        rgba
    }

    #[allow(clippy::too_many_arguments)]
    fn push_glyph_instance(
        &mut self,
        instances: &mut Vec<CellInstance>,
        bitmap: &crate::text::GlyphBitmap,
        key: crate::text::GlyphKey,
        cell_x: f32,
        cell_y: f32,
        font_size: f32,
        fg: [f32; 4],
    ) {
        if bitmap.width == 0 || bitmap.height == 0 {
            return;
        }
        if let Some((uv, is_new)) = self.atlas.get_or_insert(key, bitmap.width, bitmap.height) {
            if is_new {
                let rgba = Self::gray_to_rgba(&bitmap.data);
                self.atlas.upload_glyph(&self.queue, uv, &rgba, bitmap.width, bitmap.height);
            }
            // Baseline must be derived from the *per-glyph* font size, not
            // the cached cell_h (which reflects the main buffer's font size).
            // Using the global cell_h here meant tab bar glyphs (12pt) were
            // placed with the offset for 14pt cells, drifting them down and
            // creating phantom shapes outside the tab.
            let line_h = font_size * 1.2;
            let baseline = cell_y + line_h * 0.8;
            instances.push(CellInstance {
                cell_pos: [
                    cell_x + bitmap.left as f32,
                    baseline - (bitmap.top + bitmap.height as i32) as f32,
                ],
                cell_size: [bitmap.width as f32, bitmap.height as f32],
                uv_rect: [uv.u0, uv.v0, uv.u1, uv.v1],
                fg_color: fg,
                bg_color: [0.0, 0.0, 0.0, 0.0],
            });
        }
    }

    #[allow(clippy::type_complexity)]
    fn build_simple_instances(
        &mut self,
        cells: &[(char, f32, f32, f32, [f32; 4], [f32; 4])],
    ) -> Vec<CellInstance> {
        let mut instances: Vec<CellInstance> = Vec::with_capacity(cells.len());

        for &(c, x, y, font_size, fg, _bg) in cells {
            if c == ' ' {
                continue;
            }

            let key = crate::text::GlyphKey::new(c, font_size, false, false);
            let bitmap = self.text.rasterize(key);
            self.push_glyph_instance(&mut instances, &bitmap, key, x, y, font_size, fg);
        }

        instances
    }

    /// Build instances using HarfBuzz shaping for ligature detection.
    /// Consecutive same-row same-style cells are grouped into runs; if shaping
    /// produces fewer glyphs than input characters a ligature is rendered.
    #[allow(clippy::type_complexity)]
    fn build_ligature_instances(
        &mut self,
        cells: &[(char, f32, f32, f32, [f32; 4], [f32; 4])],
    ) -> Vec<CellInstance> {
        let cell_w = self.cell_w;
        if cell_w <= 0.0 {
            return self.build_simple_instances(cells);
        }

        let mut instances: Vec<CellInstance> = Vec::with_capacity(cells.len());
        let mut i = 0;
        while i < cells.len() {
            let (c0, x0, y0, fs0, fg0, _) = cells[i];

            // Build a consecutive horizontal run of same-style cells.
            let mut run_text = String::new();
            run_text.push(c0);
            let mut j = i + 1;
            while j < cells.len() {
                let (c1, x1, y1, fs1, fg1, _) = cells[j];
                let expected_x = x0 + (j - i) as f32 * cell_w;
                if (y1 - y0).abs() < 0.5
                    && (fs1 - fs0).abs() < 0.01
                    && fg1 == fg0
                    && (x1 - expected_x).abs() < cell_w * 0.15
                {
                    run_text.push(c1);
                    j += 1;
                } else {
                    break;
                }
            }

            let run_len = j - i;
            let mut ligature_rendered = false;

            if run_len >= 2 {
                let shaped = self.text.shape_run(&run_text, fs0, false, false);
                // Always render via shaped glyph IDs — handles both:
                //   liga: count drops (multiple chars → 1 merged glyph)
                //   calt: count stays, IDs change (e.g. JetBrains Mono -> =>)
                let mut x_cursor = x0;
                for sg in &shaped {
                    if sg.glyph_id == 0 {
                        x_cursor += cell_w;
                        continue;
                    }
                    let key = crate::text::ShapedGlyphKey {
                        glyph_id: sg.glyph_id,
                        font_size_bits: fs0.to_bits(),
                        bold: false,
                        italic: false,
                    };
                    let bitmap = self.text.rasterize_glyph_id(sg.glyph_id, fs0, false, false);
                    self.push_shaped_instance(
                        &mut instances,
                        &bitmap,
                        key,
                        x_cursor + sg.x_offset,
                        y0,
                        fs0,
                        fg0,
                    );
                    x_cursor += sg.x_advance.max(cell_w);
                }
                i = j;
                ligature_rendered = true;
            }

            if !ligature_rendered {
                let (c, x, y, font_size, fg, _) = cells[i];
                if c != ' ' {
                    let key = crate::text::GlyphKey::new(c, font_size, false, false);
                    let bitmap = self.text.rasterize(key);
                    self.push_glyph_instance(&mut instances, &bitmap, key, x, y, font_size, fg);
                }
                i += 1;
            }
        }

        instances
    }

    #[allow(clippy::too_many_arguments)]
    fn push_shaped_instance(
        &mut self,
        instances: &mut Vec<CellInstance>,
        bitmap: &crate::text::GlyphBitmap,
        key: crate::text::ShapedGlyphKey,
        cell_x: f32,
        cell_y: f32,
        font_size: f32,
        fg: [f32; 4],
    ) {
        if bitmap.width == 0 || bitmap.height == 0 {
            return;
        }
        if let Some((uv, is_new)) =
            self.atlas.get_or_insert_shaped(key, bitmap.width, bitmap.height)
        {
            if is_new {
                let rgba = Self::gray_to_rgba(&bitmap.data);
                self.atlas.upload_glyph(&self.queue, uv, &rgba, bitmap.width, bitmap.height);
            }
            let line_h = font_size * 1.2;
            let baseline = cell_y + line_h * 0.8;
            instances.push(CellInstance {
                cell_pos: [
                    cell_x + bitmap.left as f32,
                    baseline - (bitmap.top + bitmap.height as i32) as f32,
                ],
                cell_size: [bitmap.width as f32, bitmap.height as f32],
                uv_rect: [uv.u0, uv.v0, uv.u1, uv.v1],
                fg_color: fg,
                bg_color: [0.0, 0.0, 0.0, 0.0],
            });
        }
    }

    #[allow(clippy::type_complexity)]
    pub fn draw_frame(
        &mut self,
        cells: &[(char, f32, f32, f32, [f32; 4], [f32; 4])],
        ui_rects: &[UIRect],
        bg_rects: &[UIRect],
    ) {
        self.draw_frame_with_options(cells, ui_rects, bg_rects, false, true, true);
    }

    /// Draw a frame, conditionally skipping GPU uploads when data hasn't changed.
    ///
    /// `cells_dirty` — rebuild and upload cell instances (atlas lookups + vertex buffer write).
    /// `ui_dirty`    — upload bg_rects and ui_rects to GPU.
    /// When both are false the render pass re-uses the buffers from the previous frame.
    #[allow(clippy::type_complexity)]
    pub fn draw_frame_with_options(
        &mut self,
        cells: &[(char, f32, f32, f32, [f32; 4], [f32; 4])],
        ui_rects: &[UIRect],
        bg_rects: &[UIRect],
        ligatures: bool,
        cells_dirty: bool,
        ui_dirty: bool,
    ) {
        if cells_dirty {
            self.atlas.begin_frame();
            self.cached_instances = if ligatures {
                self.build_ligature_instances(cells)
            } else {
                self.build_simple_instances(cells)
            };
            // Disjoint field borrows: cell_renderer ≠ cached_instances ≠ queue.
            let inst = self.cached_instances.as_slice();
            self.cell_renderer.upload(inst, &self.queue);
        }
        if ui_dirty {
            self.ui_renderer_bg.upload(bg_rects, &self.queue);
            self.ui_renderer.upload(ui_rects, &self.queue);
        }
        self.render_instances();
    }

    fn render_instances(&mut self) {
        self.cell_renderer
            .update_screen_size(&self.queue, self.size.width, self.size.height);
        self.ui_renderer_bg
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
                label: Some("SYNAPSE_ Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SYNAPSE_ Render Pass"),
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

            // bg layer: selection, search highlights, colored cell backgrounds
            self.ui_renderer_bg.draw(&mut render_pass);

            // glyph layer (transparent bg so bitmaps that overflow cell bounds blend correctly)
            self.cell_renderer.draw(&mut render_pass, &self.atlas.bind_group);

            // overlay layer: cursor, tab bar, pane borders
            self.ui_renderer.draw(&mut render_pass);
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();
    }
}
