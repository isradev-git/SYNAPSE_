# Underline Styles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render all 5 VT underline styles (line, double, curl, dot, dash) + per-cell SGR 58 color, making Neovim diagnostics/spell-check visible.

**Architecture:** New `UnderlineRenderer` with its own wgpu pipeline and WGSL shader. Separate from `UIRenderer` to avoid regression risk. Cell flag snapshot taken inside the Term grid lock; span merging + instance building happen outside the lock. Underlines uploaded with the same dirty trigger as cells (both derived from grid data).

**Tech Stack:** Rust, wgpu 22, WGSL, alacritty_terminal 0.24, bytemuck

---

## File Map

| Action | Path | Responsibility |
|--------|------|----------------|
| NEW | `crates/SYNAPSE_-renderer/src/shaders/underline.wgsl` | WGSL shader: 5 style patterns |
| NEW | `crates/SYNAPSE_-renderer/src/underline.rs` | `UnderlineInstance`, `UnderlineRenderer` |
| MOD | `crates/SYNAPSE_-renderer/src/lib.rs` | expose `underline` module |
| MOD | `crates/SYNAPSE_-renderer/src/renderer.rs` | add field, wire draw_frame + render pass |
| MOD | `crates/SYNAPSE_-app/src/render.rs` | `build_underline_spans`, snapshot, draw call |
| MOD | `crates/SYNAPSE_-app/src/app.rs` | `cached_underline_instances` field |

---

## Task 1: WGSL Shader

**Files:**
- Create: `crates/SYNAPSE_-renderer/src/shaders/underline.wgsl`

- [ ] **Step 1: Create shader file**

```wgsl
// crates/SYNAPSE_-renderer/src/shaders/underline.wgsl

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
    @location(1) @interpolate(flat) style: u32,
    @location(2) uv: vec2<f32>,
}

struct ScreenUniform {
    screen_size: vec2<f32>,
}

@group(0) @binding(0) var<uniform> screen: ScreenUniform;

fn corner_for_index(idx: u32) -> vec2<f32> {
    if (idx == 0u) { return vec2(0.0, 0.0); }
    if (idx == 1u) { return vec2(1.0, 0.0); }
    if (idx == 2u) { return vec2(0.0, 1.0); }
    return vec2(1.0, 1.0);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) style: u32,
) -> VertexOutput {
    let corner = corner_for_index(vertex_index);
    let pixel_pos = pos + corner * size;
    var out: VertexOutput;
    out.position = vec4(
        (pixel_pos.x / screen.screen_size.x) * 2.0 - 1.0,
        1.0 - (pixel_pos.y / screen.screen_size.y) * 2.0,
        0.0,
        1.0,
    );
    out.color = color;
    out.style = style;
    out.uv = corner;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    var keep = false;

    if (in.style == 0u) {
        // solid underline
        keep = true;
    } else if (in.style == 1u) {
        // double: two horizontal bands
        keep = uv.y < 0.35 || uv.y > 0.65;
    } else if (in.style == 2u) {
        // undercurl: sine wave
        let wave = sin(uv.x * 2.5 * 6.2832);
        keep = abs(uv.y - (wave * 0.3 + 0.5)) < 0.18;
    } else if (in.style == 3u) {
        // dotted: circular dots, radius 40% of period
        let dx = fract(uv.x / 0.25) - 0.5;
        let dy = uv.y - 0.5;
        keep = sqrt(dx * dx + dy * dy) < 0.2;
    } else {
        // dashed (style == 4): 55% on, 45% off
        keep = fract(uv.x * 4.0) < 0.55;
    }

    if (!keep) { discard; }
    return in.color;
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/SYNAPSE_-renderer/src/shaders/underline.wgsl
git commit -m "feat(renderer): add underline WGSL shader with 5 style patterns"
```

---

## Task 2: UnderlineInstance + UnderlineRenderer

**Files:**
- Create: `crates/SYNAPSE_-renderer/src/underline.rs`

- [ ] **Step 1: Write failing size test**

Add to the bottom of the (not yet created) `underline.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_size_is_48_bytes() {
        assert_eq!(std::mem::size_of::<UnderlineInstance>(), 48);
    }
}
```

- [ ] **Step 2: Create `underline.rs`**

```rust
// crates/SYNAPSE_-renderer/src/underline.rs
use std::sync::Arc;
use wgpu::{BindGroup, Buffer, Device, Queue, RenderPipeline};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UnderlineInstance {
    pub pos:   [f32; 2],  // top-left of underline rect, in pixels
    pub size:  [f32; 2],  // width × height
    pub color: [f32; 4],  // RGBA
    pub style: u32,       // 0=line 1=double 2=curl 3=dot 4=dash
    pub _pad:  [u32; 3],  // padding to 48 bytes total
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
```

- [ ] **Step 3: Run test to verify struct size**

```bash
cargo test -p synapse_renderer underline::tests -- --nocapture
```

Expected: PASS — `instance_size_is_48_bytes` green.

- [ ] **Step 4: Commit**

```bash
git add crates/SYNAPSE_-renderer/src/underline.rs
git commit -m "feat(renderer): add UnderlineInstance + UnderlineRenderer"
```

---

## Task 3: Expose via lib.rs

**Files:**
- Modify: `crates/SYNAPSE_-renderer/src/lib.rs`

- [ ] **Step 1: Add module declaration**

Current `lib.rs`:
```rust
pub mod atlas;
pub mod cell;
pub mod image;
pub mod renderer;
pub mod text;
pub mod ui;
```

New `lib.rs`:
```rust
pub mod atlas;
pub mod cell;
pub mod image;
pub mod renderer;
pub mod text;
pub mod ui;
pub mod underline;
```

- [ ] **Step 2: Verify build**

```bash
cargo build -p synapse_renderer
```

Expected: compiles cleanly, no warnings.

- [ ] **Step 3: Commit**

```bash
git add crates/SYNAPSE_-renderer/src/lib.rs
git commit -m "feat(renderer): expose underline module"
```

---

## Task 4: Wire UnderlineRenderer into Renderer

**Files:**
- Modify: `crates/SYNAPSE_-renderer/src/renderer.rs`

- [ ] **Step 1: Add import and field to Renderer struct**

In the `use` block at the top of `renderer.rs`, add:
```rust
use crate::underline::{UnderlineInstance, UnderlineRenderer};
```

In the `Renderer` struct (after `image_renderer: ImageRenderer,`), add:
```rust
underline_renderer: UnderlineRenderer,
```

- [ ] **Step 2: Construct in `Renderer::new`**

After the line `let image_renderer = ImageRenderer::new(Arc::clone(&device), config.format);`, add:
```rust
let underline_renderer = UnderlineRenderer::new(Arc::clone(&device), config.format);
```

In the `Ok(Self { ... })` block, add:
```rust
underline_renderer,
```

- [ ] **Step 3: Add `underlines` param to `draw_frame` and `draw_frame_with_options`**

Replace `draw_frame` signature:
```rust
pub fn draw_frame(
    &mut self,
    cells: &[(char, f32, f32, f32, [f32; 4], [f32; 4])],
    ui_rects: &[UIRect],
    bg_rects: &[UIRect],
    underlines: &[UnderlineInstance],
    images: &[ImageInstance],
    image_ids: &[u32],
    image_clips: &[[u32; 4]],
) {
    self.draw_frame_with_options(
        cells, ui_rects, bg_rects, underlines,
        images, image_ids, image_clips, false, true, true,
    );
}
```

Replace `draw_frame_with_options` signature:
```rust
pub fn draw_frame_with_options(
    &mut self,
    cells: &[(char, f32, f32, f32, [f32; 4], [f32; 4])],
    ui_rects: &[UIRect],
    bg_rects: &[UIRect],
    underlines: &[UnderlineInstance],
    images: &[ImageInstance],
    image_ids: &[u32],
    image_clips: &[[u32; 4]],
    ligatures: bool,
    cells_dirty: bool,
    ui_dirty: bool,
) {
```

Inside `draw_frame_with_options`, in the `if cells_dirty {` block, after `self.cell_renderer.upload(inst, &self.queue);`, add:
```rust
self.underline_renderer.upload(underlines, &self.queue);
```

- [ ] **Step 4: Wire into `render_instances`**

In `render_instances`, after the other `update_screen_size` calls:
```rust
self.underline_renderer
    .update_screen_size(&self.queue, self.size.width, self.size.height);
```

In the render pass block, between `self.cell_renderer.draw(...)` and `self.ui_renderer.draw(...)`, add:
```rust
// underline layer: between glyphs and cursor/border overlays
self.underline_renderer.draw(&mut render_pass);
```

- [ ] **Step 5: Fix `draw_cells` and `draw_text` callers**

Both call `self.render_instances(...)` after uploading empty `ui_rects`. They don't need underlines. Add an upload of empty slice after those functions:

In `draw_cells`:
```rust
self.underline_renderer.upload(&[], &self.queue);
```

In `draw_text`:
```rust
self.underline_renderer.upload(&[], &self.queue);
```

- [ ] **Step 6: Build to verify**

```bash
cargo build -p synapse_renderer
```

Expected: compiles. (render.rs in SYNAPSE_-app will fail — fix in Task 6.)

- [ ] **Step 7: Commit**

```bash
git add crates/SYNAPSE_-renderer/src/renderer.rs
git commit -m "feat(renderer): wire UnderlineRenderer into draw pipeline"
```

---

## Task 5: `build_underline_spans` + Tests

**Files:**
- Modify: `crates/SYNAPSE_-app/src/render.rs`

- [ ] **Step 1: Write failing tests**

Add at the bottom of `render.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use synapse_renderer::underline::UnderlineInstance;

    fn inst(col: usize, row: i32, w: usize, style: u32, color: [f32; 4]) -> UnderlineInstance {
        let (y_off, h) = match style {
            1 => (16.0_f32 - 3.5, 3.0_f32),
            2 => (16.0_f32 - 4.0, 4.0_f32),
            _ => (16.0_f32 - 2.0, 1.5_f32),
        };
        UnderlineInstance {
            pos:   [col as f32 * 8.0, row as f32 * 16.0 + y_off],
            size:  [w as f32 * 8.0, h],
            color,
            style,
            _pad:  [0; 3],
        }
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(build_underline_spans(&[], 0.0, 0.0, 8.0, 16.0).is_empty());
    }

    #[test]
    fn single_cell_one_instance() {
        let color = [1.0f32, 0.0, 0.0, 1.0];
        let result = build_underline_spans(&[(0, 0, 0, color)], 0.0, 0.0, 8.0, 16.0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].style, 0);
        assert!((result[0].size[0] - 8.0).abs() < 0.001);
    }

    #[test]
    fn consecutive_same_style_merged() {
        let color = [1.0f32, 0.0, 0.0, 1.0];
        let cells = [(0, 0, 2, color), (1, 0, 2, color), (2, 0, 2, color)];
        let result = build_underline_spans(&cells, 0.0, 0.0, 8.0, 16.0);
        assert_eq!(result.len(), 1);
        assert!((result[0].size[0] - 24.0).abs() < 0.001);
    }

    #[test]
    fn different_style_breaks_span() {
        let color = [1.0f32, 0.0, 0.0, 1.0];
        let cells = [(0, 0, 0, color), (1, 0, 2, color)];
        let result = build_underline_spans(&cells, 0.0, 0.0, 8.0, 16.0);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].style, 0);
        assert_eq!(result[1].style, 2);
    }

    #[test]
    fn different_row_breaks_span() {
        let color = [1.0f32, 0.0, 0.0, 1.0];
        let cells = [(0, 0, 0, color), (1, 1, 0, color)];
        let result = build_underline_spans(&cells, 0.0, 0.0, 8.0, 16.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn undercurl_y_offset_and_height() {
        let cells = [(0, 0, 2, [1.0f32, 0.0, 0.0, 1.0])];
        let result = build_underline_spans(&cells, 0.0, 0.0, 8.0, 16.0);
        assert_eq!(result.len(), 1);
        // y = 0.0 + 0 * 16.0 + (16.0 - 4.0) = 12.0
        assert!((result[0].pos[1] - 12.0).abs() < 0.001);
        assert!((result[0].size[1] - 4.0).abs() < 0.001);
    }

    #[test]
    fn non_consecutive_column_breaks_span() {
        let color = [1.0f32, 0.0, 0.0, 1.0];
        let cells = [(0, 0, 0, color), (2, 0, 0, color)]; // col gap
        let result = build_underline_spans(&cells, 0.0, 0.0, 8.0, 16.0);
        assert_eq!(result.len(), 2);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p SYNAPSE_-app -- underline 2>&1 | head -30
```

Expected: FAIL — `build_underline_spans` not found.

- [ ] **Step 3: Implement `build_underline_spans`**

Add this function to `render.rs` (near the top, after the color conversion helpers):

```rust
use synapse_renderer::underline::UnderlineInstance;

pub(crate) fn build_underline_spans(
    ul_cells: &[(usize, i32, u32, [f32; 4])],
    content_x: f32,
    content_y: f32,
    cell_w: f32,
    cell_h: f32,
) -> Vec<UnderlineInstance> {
    let mut result = Vec::new();
    if ul_cells.is_empty() {
        return result;
    }

    let make = |col: usize, row: i32, w: usize, style: u32, color: [f32; 4]| {
        let (y_off, h) = match style {
            1 => (cell_h - 3.5, 3.0_f32),
            2 => (cell_h - 4.0, 4.0_f32),
            _ => (cell_h - 2.0, 1.5_f32),
        };
        UnderlineInstance {
            pos:   [content_x + col as f32 * cell_w, content_y + row as f32 * cell_h + y_off],
            size:  [w as f32 * cell_w, h],
            color,
            style,
            _pad:  [0; 3],
        }
    };

    let (mut s_col, mut s_row, mut s_style, mut s_color) =
        (ul_cells[0].0, ul_cells[0].1, ul_cells[0].2, ul_cells[0].3);
    let mut span_w = 1usize;

    for &(col, row, style, color) in &ul_cells[1..] {
        let extends = row == s_row
            && col == s_col + span_w
            && style == s_style
            && color == s_color;
        if extends {
            span_w += 1;
        } else {
            result.push(make(s_col, s_row, span_w, s_style, s_color));
            (s_col, s_row, s_style, s_color) = (col, row, style, color);
            span_w = 1;
        }
    }
    result.push(make(s_col, s_row, span_w, s_style, s_color));
    result
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p SYNAPSE_-app -- tests:: -- --nocapture
```

Expected: all 7 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/SYNAPSE_-app/src/render.rs
git commit -m "feat(app): add build_underline_spans with span merging + tests"
```

---

## Task 6: AppCore Field + render_frame Wiring

**Files:**
- Modify: `crates/SYNAPSE_-app/src/app.rs`
- Modify: `crates/SYNAPSE_-app/src/render.rs`

- [ ] **Step 1: Add `cached_underline_instances` to AppCore**

In `app.rs`, add import at top:
```rust
use synapse_renderer::underline::UnderlineInstance;
```

In `AppCore` struct (after `cached_bg_rects: Vec<UIRect>`), add:
```rust
pub cached_underline_instances: Vec<UnderlineInstance>,
```

In `AppCore::new` (or wherever `AppCore` is constructed), after `cached_bg_rects: Vec::new()`:
```rust
cached_underline_instances: Vec::new(),
```

In the clear block (line ~276, after `self.cached_bg_rects.clear()`):
```rust
self.cached_underline_instances.clear();
```

- [ ] **Step 2: Add param to `render_frame` signature**

In `render.rs`, add `cached_underline_instances: &mut Vec<UnderlineInstance>` to `render_frame` after `cached_bg_rects`:

```rust
pub fn render_frame(
    renderer: &mut Renderer,
    layout: &Layout,
    tab_bar: &mut TabBar,
    panes: &mut Vec<Pane>,
    image_store: &ImageStore,
    state: &AppState,
    cell_w: f32,
    cell_h: f32,
    cursor_blink_on: bool,
    cached_cell_data: &mut CellData,
    cached_ui_rects: &mut Vec<UIRect>,
    cached_bg_rects: &mut Vec<UIRect>,
    cached_underline_instances: &mut Vec<UnderlineInstance>,   // NEW
    cached_blink: &mut bool,
    cached_font_size: &mut f32,
    cached_active_tab: &mut usize,
    cached_cursor_rects_start: &mut usize,
    cached_cursor_pixel: &mut Option<(f32, f32)>,
    cached_url_spans: &mut Vec<UrlSpan>,
    effective_font_size: f32,
    scale_factor: f32,
) -> Vec<PaneId> {
```

- [ ] **Step 3: Clear + rebuild underlines in the `needs_cell_rebuild` block**

In the `if needs_cell_rebuild {` block (line ~465), after `cached_url_spans.clear()`:
```rust
cached_underline_instances.clear();
```

Inside the pane loop, add a `ul_buf` before the grid lock and fill it inside the lock:

```rust
// Before the `let (cells, ...) = { ... }` block, add:
// TermColor is already imported as `use alacritty_terminal::vte::ansi::Color as TermColor`
let mut ul_buf: Vec<(usize, i32, u32, Option<TermColor>)> = Vec::new();
```

First, add to the import block at the top of `render.rs` (alongside existing `use alacritty_terminal::...` lines):
```rust
use alacritty_terminal::term::cell::Flags;
```

Inside the grid lock block, in the `for indexed in grid.display_iter()` loop, after the
existing `buf.push(...)` call, add:

```rust
let flags = indexed.flags;
if flags.intersects(Flags::ALL_UNDERLINES) {
    let style = if flags.contains(Flags::UNDERCURL)         { 2u32 }
           else if flags.contains(Flags::DOUBLE_UNDERLINE)  { 1 }
           else if flags.contains(Flags::DOTTED_UNDERLINE)  { 3 }
           else if flags.contains(Flags::DASHED_UNDERLINE)  { 4 }
           else                                             { 0 };
    ul_buf.push((col, viewport_row, style, indexed.underline_color()));
}
```

After the lock block closes (still inside the pane loop), add:

```rust
if !ul_buf.is_empty() {
    let ul_cells: Vec<(usize, i32, u32, [f32; 4])> = ul_buf
        .into_iter()
        .map(|(col, viewport_row, style, ul_color)| {
            let rgba = ul_color
                .map(|c| term_color_to_rgba(c, state.theme.fg, &state.theme.ansi_colors))
                .unwrap_or(state.theme.fg);
            (col, viewport_row, style, rgba)
        })
        .collect();
    let spans = build_underline_spans(&ul_cells, content_x, content_y, cell_w, cell_h);
    cached_underline_instances.extend(spans);
}
```

- [ ] **Step 4: Pass underlines to `draw_frame_with_options`**

Replace the `renderer.draw_frame_with_options(...)` call at line ~1013:
```rust
renderer.draw_frame_with_options(
    cached_cell_data,
    cached_ui_rects,
    cached_bg_rects,
    cached_underline_instances,       // NEW
    &image_draws,
    &image_draw_ids,
    &image_clips,
    state.config.font_ligatures,
    needs_cell_rebuild,
    needs_ui_rebuild,
);
```

Also update `render_splash_screen` (calls `draw_frame_with_options` at line ~1149):
```rust
renderer.draw_frame_with_options(
    &cells,
    &ui_rects,
    &bg_rects,
    &[],              // underlines: empty for splash
    &[], &[], &[],
    false, true, true,
)
```

- [ ] **Step 5: Pass `cached_underline_instances` at the render_frame call site in app.rs**

In `app.rs` at the `render_frame(...)` call (~line 1209), add after `&mut self.cached_bg_rects,`:
```rust
&mut self.cached_underline_instances,
```

- [ ] **Step 6: Build the workspace**

```bash
cargo build -p SYNAPSE_-app
```

Expected: compiles cleanly.

- [ ] **Step 7: Clippy**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: zero warnings. Fix any that appear before continuing.

- [ ] **Step 8: Run all tests**

```bash
cargo test --workspace
```

Expected: all ~80 tests pass.

- [ ] **Step 9: Commit**

```bash
git add crates/SYNAPSE_-app/src/app.rs crates/SYNAPSE_-app/src/render.rs
git commit -m "feat(app): wire underline instances through render_frame and draw pipeline"
```

---

## Task 7: Smoke Test

- [ ] **Step 1: Run SYNAPSE_ and open Neovim**

```bash
cargo run -p SYNAPSE_-app
```

In the terminal, run:
```bash
nvim
```

- [ ] **Step 2: Trigger undercurl via Neovim spell check**

In Neovim command mode:
```
:set spell
```

Deliberately misspell a word. Expected: red undercurl visible under the misspelled word.

- [ ] **Step 3: Trigger LSP diagnostics (if LSP configured)**

Open a file with a warning. Expected: colored undercurl under the diagnostic location, color matching the LSP severity (error=red, warning=yellow, etc.).

- [ ] **Step 4: Verify other styles with a test script**

Run this in the terminal:
```bash
printf "\e[4munderline\e[0m "
printf "\e[4:2mdouble\e[0m "
printf "\e[4:3mundercurl\e[0m "
printf "\e[4:4mdotted\e[0m "
printf "\e[4:5mdashed\e[0m\n"
printf "\e[4:3m\e[58;2;255;100;100mcolored curl\e[0m\n"
```

Expected: 5 visually distinct underline styles + one colored undercurl.
