# SYNAPSE_ v2 Phase 1 — Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `SYNAPSE_-terminal` with `alacritty_terminal` and `cosmic-text/swash` with `fontdue`, producing a terminal that renders glyphs correctly with no atlas/pixel artifacts.

**Architecture:** `alacritty_terminal::Term<EventProxy>` owns all VT state and the scrollback grid. A reader thread feeds PTY bytes into `Term` via `Processor::advance`. `fontdue::Font` rasterizes glyphs to grayscale bitmaps; `gray_to_rgba()` converts them before atlas upload. The wgpu pipeline and shaders are untouched.

**Tech Stack:** `alacritty_terminal = "0.24"`, `fontdue = "0.9"`, `wgpu = "22"`, `winit = "0.30"`, `portable-pty = "0.8"`

---

## File Map

| Action | Path | Responsibility |
|--------|------|----------------|
| Delete | `crates/SYNAPSE_-terminal/` | already removed in cleanup |
| Modify | `Cargo.toml` | remove SYNAPSE_-terminal member + cosmic-text/vte workspace deps; add fontdue + alacritty_terminal |
| Modify | `crates/SYNAPSE_-renderer/Cargo.toml` | swap cosmic-text → fontdue |
| Modify | `crates/SYNAPSE_-renderer/src/text.rs` | fontdue-based rasterizer + cell metrics |
| Modify | `crates/SYNAPSE_-renderer/src/atlas.rs` | swap `CacheKey` → `GlyphKey`; add `gray_to_rgba` helper |
| Modify | `crates/SYNAPSE_-renderer/src/renderer.rs` | use `GlyphBitmap`/`GlyphKey`; remove `swash_to_rgba` |
| Modify | `crates/SYNAPSE_-ui/Cargo.toml` | remove synapse_terminal; add alacritty_terminal + portable-pty |
| Modify | `crates/SYNAPSE_-ui/src/pane.rs` | new Pane: holds `Term<EventProxy>` + PTY writer + dirty flag |
| Modify | `crates/SYNAPSE_-app/Cargo.toml` | remove SYNAPSE_-terminal; keep portable-pty (already transitive) |
| Modify | `crates/SYNAPSE_-app/src/pane_ops.rs` | rewrite `create_pane()` for alacritty_terminal |
| Modify | `crates/SYNAPSE_-app/src/render.rs` | read grid via `term.grid().display_iter()` |
| Modify | `crates/SYNAPSE_-app/src/app.rs` | update PTY write, resize, focus event mode checks |

---

## Task 1: Update Cargo.toml files

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/SYNAPSE_-renderer/Cargo.toml`
- Modify: `crates/SYNAPSE_-ui/Cargo.toml`
- Modify: `crates/SYNAPSE_-app/Cargo.toml`

- [ ] **Step 1: Update workspace Cargo.toml**

Replace the entire file content:

```toml
[workspace]
members = [
    "crates/SYNAPSE_-app",
    "crates/SYNAPSE_-renderer",
    "crates/SYNAPSE_-ui",
    "crates/SYNAPSE_-config",
]
resolver = "2"

[workspace.package]
version = "0.2.0"
edition = "2021"
license = "MIT"
authors = ["SYNAPSE_ Team"]
repository = "https://github.com/isradev-git/luna"
homepage = "https://github.com/isradev-git/luna"
description = "A modern GPU-accelerated terminal emulator"

[workspace.dependencies]
winit = "0.30"
wgpu = "22"
fontdue = "0.9"
alacritty_terminal = "0.24"
portable-pty = "0.8"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
toml = "0.8"
arboard = "3"
tracing = "0.1"
tracing-subscriber = "0.3"
bitflags = "2"
clap = { version = "4", features = ["derive"] }
pollster = "0.3"
bytemuck = { version = "1", features = ["derive"] }

[profile.dist]
inherits = "release"
lto = "thin"
```

- [ ] **Step 2: Update SYNAPSE_-renderer/Cargo.toml**

```toml
[package]
name = "SYNAPSE_-renderer"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true

[lib]
name = "synapse_renderer"
path = "src/lib.rs"

[dependencies]
wgpu = { workspace = true }
winit = { workspace = true }
fontdue = { workspace = true }
tracing = { workspace = true }
pollster = { workspace = true }
bytemuck = { workspace = true }
```

- [ ] **Step 3: Update SYNAPSE_-ui/Cargo.toml**

```toml
[package]
name = "SYNAPSE_-ui"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true

[lib]
name = "synapse_ui"
path = "src/lib.rs"

[dependencies]
tracing = { workspace = true }
alacritty_terminal = { workspace = true }
portable-pty = { workspace = true }
SYNAPSE_-renderer = { path = "../SYNAPSE_-renderer" }
winit = { workspace = true }
```

- [ ] **Step 4: Update SYNAPSE_-app/Cargo.toml**

```toml
[package]
name = "SYNAPSE_-app"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "SYNAPSE_ terminal emulator — binary entry point"

[[bin]]
name = "synapse_"
path = "src/main.rs"

[dependencies]
SYNAPSE_-renderer = { path = "../SYNAPSE_-renderer" }
SYNAPSE_-ui = { path = "../SYNAPSE_-ui" }
SYNAPSE_-config = { path = "../SYNAPSE_-config" }
alacritty_terminal = { workspace = true }
winit = { workspace = true }
wgpu = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
clap = { workspace = true }
arboard = { workspace = true }
```

- [ ] **Step 5: Run cargo fetch to check deps resolve**

```bash
cargo fetch
```

Expected: resolves without error. If `alacritty_terminal = "0.24"` fails, run `cargo search alacritty_terminal` to find the latest published version and update accordingly.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/SYNAPSE_-renderer/Cargo.toml crates/SYNAPSE_-ui/Cargo.toml crates/SYNAPSE_-app/Cargo.toml Cargo.lock
git commit -m "chore(deps): swap cosmic-text→fontdue, add alacritty_terminal, remove SYNAPSE_-terminal"
```

---

## Task 2: Verify alacritty_terminal API

**Files:**
- Read: `target/doc/alacritty_terminal/index.html` (after generating)

- [ ] **Step 1: Generate docs**

```bash
cargo doc --package alacritty_terminal --no-deps 2>&1 | tail -5
```

Expected: `Finished` with no errors.

- [ ] **Step 2: Confirm the following types exist**

Open `target/doc/alacritty_terminal/index.html` or grep:

```bash
grep -r "fn advance" target/doc/alacritty_terminal/ 2>/dev/null | head -5
grep -r "display_iter" target/doc/alacritty_terminal/ 2>/dev/null | head -5
grep -r "struct Term" target/doc/alacritty_terminal/ 2>/dev/null | head -5
```

**Required types for this plan:**
- `alacritty_terminal::event::EventListener` (trait)
- `alacritty_terminal::event::Event` (enum)
- `alacritty_terminal::term::Term<T>` (struct)
- `alacritty_terminal::term::Term::new(config, size, proxy)` (constructor)
- `alacritty_terminal::grid::Dimensions` (trait) — or equivalent sizing trait
- A `Processor` type with an `advance(&mut Term, u8)` method
- `term.grid().display_iter()` yielding cells with `.point` and `.c`, `.fg`, `.bg`
- `alacritty_terminal::vte::ansi::Color` or equivalent color enum with `Spec(Rgb)`, `Named(...)`, `Indexed(u8)` variants

- [ ] **Step 3: Note actual import paths**

If any path differs from what this plan uses, note the correct path and apply the correction to all subsequent tasks before implementing them. The most common difference is the exact module path for `Color`, `Processor`, and `Dimensions`.

---

## Task 3: Rewrite text.rs with fontdue

**Files:**
- Modify: `crates/SYNAPSE_-renderer/src/text.rs`
- Test: `crates/SYNAPSE_-renderer/src/text.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Write failing tests first**

Replace the entire `crates/SYNAPSE_-renderer/src/text.rs`:

```rust
const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
const JETBRAINS_MONO_BOLD: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf");
const JETBRAINS_MONO_ITALIC: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Italic.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub ch: char,
    pub font_size_bits: u32,
    pub bold: bool,
    pub italic: bool,
}

impl GlyphKey {
    pub fn new(ch: char, font_size: f32, bold: bool, italic: bool) -> Self {
        Self {
            ch,
            font_size_bits: font_size.to_bits(),
            bold,
            italic,
        }
    }
}

pub struct GlyphBitmap {
    pub width: u32,
    pub height: u32,
    pub top: i32,
    pub left: i32,
    pub advance_width: f32,
    pub data: Vec<u8>,
}

pub struct TextShaping {
    font_regular: fontdue::Font,
    font_bold: fontdue::Font,
    font_italic: fontdue::Font,
}

impl TextShaping {
    pub fn new() -> Self {
        let settings = fontdue::FontSettings::default();
        let font_regular = fontdue::Font::from_bytes(JETBRAINS_MONO_REGULAR, settings)
            .expect("embedded JetBrains Mono Regular is invalid");
        let font_bold = fontdue::Font::from_bytes(JETBRAINS_MONO_BOLD, settings)
            .expect("embedded JetBrains Mono Bold is invalid");
        let font_italic = fontdue::Font::from_bytes(JETBRAINS_MONO_ITALIC, settings)
            .expect("embedded JetBrains Mono Italic is invalid");
        Self { font_regular, font_bold, font_italic }
    }

    fn font(&self, bold: bool, italic: bool) -> &fontdue::Font {
        match (bold, italic) {
            (true, _) => &self.font_bold,
            (_, true) => &self.font_italic,
            _ => &self.font_regular,
        }
    }

    pub fn rasterize(&self, key: GlyphKey) -> GlyphBitmap {
        let font_size = f32::from_bits(key.font_size_bits);
        let font = self.font(key.bold, key.italic);
        let (metrics, data) = font.rasterize(key.ch, font_size);
        GlyphBitmap {
            width: metrics.width as u32,
            height: metrics.height as u32,
            top: metrics.ymin,
            left: metrics.xmin,
            advance_width: metrics.advance_width,
            data,
        }
    }

    pub fn cell_metrics(&self, font_size: f32) -> (f32, f32) {
        let metrics = self.font_regular.metrics('M', font_size);
        let cell_w = metrics.advance_width;
        let cell_h = font_size * 1.2;
        (cell_w, cell_h)
    }
}

impl Default for TextShaping {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rasterize_ascii_produces_bitmap() {
        let shaping = TextShaping::new();
        let key = GlyphKey::new('A', 14.0, false, false);
        let glyph = shaping.rasterize(key);
        assert!(glyph.width > 0, "width must be > 0 for 'A'");
        assert!(glyph.height > 0, "height must be > 0 for 'A'");
        assert_eq!(
            glyph.data.len(),
            (glyph.width * glyph.height) as usize,
            "bitmap length must equal width*height (grayscale)"
        );
    }

    #[test]
    fn rasterize_space_is_empty() {
        let shaping = TextShaping::new();
        let key = GlyphKey::new(' ', 14.0, false, false);
        let glyph = shaping.rasterize(key);
        assert_eq!(glyph.data.len(), 0, "space produces empty bitmap");
    }

    #[test]
    fn cell_metrics_are_positive() {
        let shaping = TextShaping::new();
        let (w, h) = shaping.cell_metrics(14.0);
        assert!(w > 0.0, "cell width must be > 0");
        assert!(h > 0.0, "cell height must be > 0");
    }

    #[test]
    fn bold_differs_from_regular() {
        let shaping = TextShaping::new();
        let regular = shaping.rasterize(GlyphKey::new('B', 14.0, false, false));
        let bold = shaping.rasterize(GlyphKey::new('B', 14.0, true, false));
        // Bitmaps may have same dimensions for some fonts but data should differ
        assert!(
            regular.data != bold.data || regular.width != bold.width,
            "bold and regular bitmaps should differ"
        );
    }
}
```

- [ ] **Step 2: Run tests (expect compile failure — cosmic_text not imported yet)**

```bash
cargo test -p SYNAPSE_-renderer --lib 2>&1 | head -30
```

Expected: compile error because renderer.rs still imports cosmic_text. That's ok — we fix renderer next.

- [ ] **Step 3: Commit text.rs**

```bash
git add crates/SYNAPSE_-renderer/src/text.rs
git commit -m "feat(renderer): replace cosmic-text with fontdue in text.rs"
```

---

## Task 4: Update atlas.rs — swap CacheKey → GlyphKey

**Files:**
- Modify: `crates/SYNAPSE_-renderer/src/atlas.rs`

- [ ] **Step 1: Replace atlas.rs content**

```rust
use std::collections::HashMap;
use wgpu::{Device, Queue};

use crate::text::GlyphKey;

pub const ATLAS_SIZE: u32 = 2048;

#[derive(Debug, Clone, Copy)]
pub struct UvRect {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

struct AtlasEntry {
    uv: UvRect,
    last_frame: u64,
}

pub struct TextureAtlas {
    pub texture: wgpu::Texture,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
    cache: HashMap<GlyphKey, AtlasEntry>,
    x_offset: u32,
    y_offset: u32,
    row_height: u32,
    frame: u64,
    needs_reset: bool,
    warned_90: bool,
}

impl TextureAtlas {
    pub fn new(device: &Device) -> Self {
        let size = wgpu::Extent3d {
            width: ATLAS_SIZE,
            height: ATLAS_SIZE,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SYNAPSE_ Glyph Atlas"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SYNAPSE_ Atlas Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SYNAPSE_ Atlas BindGroupLayout"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SYNAPSE_ Atlas BindGroup"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            texture,
            bind_group_layout,
            bind_group,
            cache: HashMap::new(),
            x_offset: 0,
            y_offset: 0,
            row_height: 0,
            frame: 0,
            needs_reset: false,
            warned_90: false,
        }
    }

    pub fn begin_frame(&mut self) {
        self.frame += 1;

        if self.needs_reset {
            let evicted = self.cache.len();
            self.cache.clear();
            self.x_offset = 0;
            self.y_offset = 0;
            self.row_height = 0;
            self.needs_reset = false;
            self.warned_90 = false;
            tracing::warn!(
                "glyph atlas reset — evicted {} cached entries (frame {})",
                evicted,
                self.frame,
            );
        }

        if !self.warned_90 {
            let fill = self.fill_fraction();
            if fill >= 0.9 {
                tracing::warn!(
                    "glyph atlas at {:.0}% capacity — will reset next frame",
                    fill * 100.0,
                );
                self.warned_90 = true;
                self.needs_reset = true;
            }
        }
    }

    fn fill_fraction(&self) -> f32 {
        (self.y_offset + self.row_height) as f32 / ATLAS_SIZE as f32
    }

    /// Returns `(uv, is_new)`. If `is_new`, caller must call `upload_glyph`.
    pub fn get_or_insert(
        &mut self,
        key: GlyphKey,
        bitmap_width: u32,
        bitmap_height: u32,
    ) -> Option<(UvRect, bool)> {
        if let Some(entry) = self.cache.get_mut(&key) {
            entry.last_frame = self.frame;
            return Some((entry.uv, false));
        }

        let rect = self.allocate(bitmap_width, bitmap_height)?;
        self.cache.insert(key, AtlasEntry { uv: rect, last_frame: self.frame });
        Some((rect, true))
    }

    fn allocate(&mut self, width: u32, height: u32) -> Option<UvRect> {
        if width == 0 || height == 0 {
            return None;
        }

        if self.x_offset + width > ATLAS_SIZE {
            self.x_offset = 0;
            self.y_offset += self.row_height;
            self.row_height = 0;
        }

        if self.y_offset + height > ATLAS_SIZE {
            self.needs_reset = true;
            return None;
        }

        let u0 = self.x_offset as f32 / ATLAS_SIZE as f32;
        let v0 = self.y_offset as f32 / ATLAS_SIZE as f32;
        let u1 = (self.x_offset + width) as f32 / ATLAS_SIZE as f32;
        let v1 = (self.y_offset + height) as f32 / ATLAS_SIZE as f32;

        self.x_offset += width;
        self.row_height = self.row_height.max(height);

        Some(UvRect { u0, v0, u1, v1 })
    }

    /// Upload RGBA bytes to the atlas at the rect returned by `get_or_insert`.
    pub fn upload_glyph(
        &mut self,
        queue: &Queue,
        rect: UvRect,
        rgba_bitmap: &[u8],
        bitmap_width: u32,
        bitmap_height: u32,
    ) {
        let x = (rect.u0 * ATLAS_SIZE as f32) as u32;
        let y = (rect.v0 * ATLAS_SIZE as f32) as u32;

        let raw_bytes_per_row = 4 * bitmap_width;
        let aligned_bytes_per_row =
            ((raw_bytes_per_row + wgpu::COPY_BYTES_PER_ROW_ALIGNMENT - 1)
                / wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
                * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

        let padded_size = (aligned_bytes_per_row * bitmap_height) as usize;
        let mut padded = vec![0u8; padded_size];
        for row in 0..bitmap_height as usize {
            let src = row * raw_bytes_per_row as usize;
            let dst = row * aligned_bytes_per_row as usize;
            padded[dst..dst + raw_bytes_per_row as usize]
                .copy_from_slice(&rgba_bitmap[src..src + raw_bytes_per_row as usize]);
        }

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &padded,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(aligned_bytes_per_row),
                rows_per_image: Some(bitmap_height),
            },
            wgpu::Extent3d {
                width: bitmap_width,
                height: bitmap_height,
                depth_or_array_layers: 1,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::GlyphKey;

    fn make_key(ch: char) -> GlyphKey {
        GlyphKey::new(ch, 14.0, false, false)
    }

    #[test]
    fn test_allocate_no_overlap() {
        let atlas_size = ATLAS_SIZE;
        let mut x_offset: u32 = 0;
        let mut y_offset: u32 = 0;
        let mut row_height: u32 = 0;
        let mut rects = Vec::new();

        for i in 0..100u32 {
            let w = (i % 5 + 1) * 8;
            let h = (i % 3 + 1) * 12;

            if x_offset + w > atlas_size {
                x_offset = 0;
                y_offset += row_height;
                row_height = 0;
            }
            assert!(y_offset + h <= atlas_size, "Atlas overflow at glyph {}", i);

            let u0 = x_offset as f32 / atlas_size as f32;
            let v0 = y_offset as f32 / atlas_size as f32;
            let u1 = (x_offset + w) as f32 / atlas_size as f32;
            let v1 = (y_offset + h) as f32 / atlas_size as f32;

            x_offset += w;
            row_height = row_height.max(h);
            rects.push((u0, v0, u1, v1));

            for (j, &(pu0, pv0, pu1, pv1)) in rects.iter().enumerate().take(rects.len() - 1) {
                let overlap = !(u0 >= pu1 || u1 <= pu0 || v0 >= pv1 || v1 <= pv0);
                assert!(!overlap, "Overlap between glyph {} and {}", i, j);
            }
        }
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/SYNAPSE_-renderer/src/atlas.rs
git commit -m "feat(renderer): swap CacheKey→GlyphKey in TextureAtlas"
```

---

## Task 5: Update renderer.rs — remove cosmic_text, use GlyphBitmap/GlyphKey

**Files:**
- Modify: `crates/SYNAPSE_-renderer/src/renderer.rs`

- [ ] **Step 1: Add gray_to_rgba helper, update push_glyph_instance**

Find and replace in `renderer.rs`:

**Remove this function entirely:**
```rust
fn swash_to_rgba(image: &cosmic_text::SwashImage) -> Vec<u8> {
```

**Add in its place:**
```rust
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
```

- [ ] **Step 2: Update push_glyph_instance signature**

Replace:
```rust
fn push_glyph_instance(
    &mut self,
    instances: &mut Vec<CellInstance>,
    image: &cosmic_text::SwashImage,
    cache_key: cosmic_text::CacheKey,
    cell_x: f32,
    cell_y: f32,
    fg: [f32; 4],
    _bg: [f32; 4],
) {
    let bw = image.placement.width;
    let bh = image.placement.height;
    if bw == 0 || bh == 0 {
        return;
    }
    if let Some((uv, is_new)) = self.atlas.get_or_insert(cache_key, bw, bh) {
        if is_new {
            let pixels = Self::swash_to_rgba(image);
            self.atlas.upload_glyph(&self.queue, uv, &pixels, bw, bh);
        }
        let baseline = cell_y + self.cell_h * 0.8;
        instances.push(CellInstance {
            cell_pos: [
                cell_x + image.placement.left as f32,
                baseline - image.placement.top as f32,
            ],
            cell_size: [bw as f32, bh as f32],
            uv_rect: [uv.u0, uv.v0, uv.u1, uv.v1],
            fg_color: fg,
            bg_color: [0.0, 0.0, 0.0, 0.0],
        });
    }
}
```

With:
```rust
fn push_glyph_instance(
    &mut self,
    instances: &mut Vec<CellInstance>,
    bitmap: &crate::text::GlyphBitmap,
    key: crate::text::GlyphKey,
    cell_x: f32,
    cell_y: f32,
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
        let baseline = cell_y + self.cell_h * 0.8;
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
```

- [ ] **Step 3: Update draw_cells and build_simple_instances to use new API**

Find `build_simple_instances` (or wherever `rasterize_glyph` is called in renderer.rs). Replace each call site like this pattern:

Old call:
```rust
if let Some((swash_image, cache_key)) = self.text.rasterize_glyph(c, font_size) {
    self.push_glyph_instance(&mut instances, &swash_image, cache_key, x, y, fg, bg);
}
```

New call:
```rust
let key = crate::text::GlyphKey::new(c, font_size, false, false);
let bitmap = self.text.rasterize(key);
self.push_glyph_instance(&mut instances, &bitmap, key, x, y, fg);
```

- [ ] **Step 4: Remove all cosmic_text imports from renderer.rs**

Delete any line starting with `use cosmic_text` at the top of the file.

- [ ] **Step 5: Remove ligature code (simplify for Phase 1)**

The `build_ligature_instances` function uses cosmic_text's shaping. Remove it entirely and remove the `font_ligatures` field usage. Replace the `if self.font_ligatures { ... } else { ... }` branch in `draw_frame` with always calling `build_simple_instances`.

Ligature support is a Phase 4 feature.

- [ ] **Step 6: Attempt compile**

```bash
cargo build -p SYNAPSE_-renderer 2>&1 | head -40
```

Fix any remaining cosmic_text references. Expected: compiles cleanly.

- [ ] **Step 7: Run renderer tests**

```bash
cargo test -p SYNAPSE_-renderer 2>&1
```

Expected: all tests pass, including `rasterize_ascii_produces_bitmap` and `test_allocate_no_overlap`.

- [ ] **Step 8: Commit**

```bash
git add crates/SYNAPSE_-renderer/src/renderer.rs
git commit -m "feat(renderer): remove cosmic-text, use fontdue GlyphBitmap"
```

---

## Task 6: Rewrite pane.rs with alacritty_terminal

**Files:**
- Modify: `crates/SYNAPSE_-ui/src/pane.rs`

- [ ] **Step 1: Replace pane.rs**

```rust
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::term::Term;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PaneId(pub u64);

#[derive(Clone)]
pub struct EventProxy {
    sender: mpsc::SyncSender<Event>,
}

impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        let _ = self.sender.try_send(event);
    }
}

pub struct Pane {
    pub id: PaneId,
    pub term: Arc<Mutex<Term<EventProxy>>>,
    pub pty_writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pub event_rx: mpsc::Receiver<Event>,
    pub dirty: Arc<AtomicBool>,
    pub cols: usize,
    pub rows: usize,
    title: String,
    cwd: String,
}

impl Pane {
    pub fn new(
        id: PaneId,
        term: Arc<Mutex<Term<EventProxy>>>,
        pty_writer: Box<dyn Write + Send>,
        event_rx: mpsc::Receiver<Event>,
        dirty: Arc<AtomicBool>,
        cols: usize,
        rows: usize,
    ) -> Self {
        Self {
            id,
            term,
            pty_writer: Arc::new(Mutex::new(pty_writer)),
            event_rx,
            dirty,
            cols,
            rows,
            title: String::new(),
            cwd: String::new(),
        }
    }

    pub fn write_to_pty(&self, data: &[u8]) {
        if let Ok(mut w) = self.pty_writer.lock() {
            let _ = w.write_all(data);
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::AcqRel)
    }

    pub fn title(&self) -> String {
        self.title.clone()
    }

    pub fn cwd(&self) -> String {
        self.cwd.clone()
    }
}
```

- [ ] **Step 2: Try to compile SYNAPSE_-ui**

```bash
cargo build -p SYNAPSE_-ui 2>&1 | head -40
```

The compile will fail because other SYNAPSE_-ui files still import synapse_terminal types. Fix imports in each file:

- `crates/SYNAPSE_-ui/src/lib.rs`: remove `synapse_terminal` from any re-exports
- Any other file in SYNAPSE_-ui that imports from `synapse_terminal`: update or remove the import

- [ ] **Step 3: Commit**

```bash
git add crates/SYNAPSE_-ui/src/pane.rs crates/SYNAPSE_-ui/src/lib.rs
git commit -m "feat(ui): replace SYNAPSE_-terminal Pane with alacritty_terminal Term"
```

---

## Task 7: Rewrite pane_ops.rs — create_pane() with alacritty_terminal

**Files:**
- Modify: `crates/SYNAPSE_-app/src/pane_ops.rs`

- [ ] **Step 1: Verify exact alacritty_terminal API for Term::new and Processor**

Before writing code, run:

```bash
grep -r "fn new" target/doc/alacritty_terminal/term/struct.Term.html 2>/dev/null | head -5
grep -r "Processor" target/doc/alacritty_terminal/ 2>/dev/null | grep "struct\|fn advance" | head -10
grep -r "trait Dimensions\|fn screen_lines\|fn columns" target/doc/alacritty_terminal/ 2>/dev/null | head -10
```

Note the exact module paths for:
- `Term::new(config, size, proxy)` — note the config type
- The `Processor` struct and its `advance(&mut term, byte)` method
- The `Dimensions` trait (needed to implement for our `TermSize`)

- [ ] **Step 2: Replace pane_ops.rs**

```rust
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::term::Term;
// NOTE: verify these exact paths with cargo doc (Task 2):
use alacritty_terminal::term::Config as TermConfig;
use alacritty_terminal::grid::Dimensions;
// Processor import path may differ — check cargo doc:
// Option A: use alacritty_terminal::ansi::Processor;
// Option B: use alacritty_terminal::vte::ansi::Processor;

use synapse_ui::pane::{EventProxy, Pane, PaneId};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};

struct TermSize {
    cols: usize,
    rows: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.rows + 10_000
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

pub fn create_pane(
    id: PaneId,
    cols: usize,
    rows: usize,
) -> Result<Pane, Box<dyn std::error::Error>> {
    let pty_system = native_pty_system();
    let pty_pair = pty_system.openpty(PtySize {
        rows: rows as u16,
        cols: cols as u16,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let cmd = CommandBuilder::new(&shell);
    let _child = pty_pair.slave.spawn_command(cmd)?;

    let pty_reader = pty_pair.master.try_clone_reader()?;
    let pty_writer = pty_pair.master.take_writer()?;

    let (event_tx, event_rx) = mpsc::sync_channel::<Event>(256);
    let proxy = EventProxy { sender: event_tx };

    let size = TermSize { cols, rows };
    let term = Term::new(TermConfig::default(), &size, proxy);
    let term = Arc::new(Mutex::new(term));

    let dirty = Arc::new(AtomicBool::new(false));

    let term_reader = Arc::clone(&term);
    let dirty_reader = Arc::clone(&dirty);
    std::thread::Builder::new()
        .name(format!("synapse_-pty-{}", id.0))
        .spawn(move || {
            let mut reader = pty_reader;
            let mut buf = [0u8; 4096];
            // NOTE: verify Processor import path (Task 2 Step 2).
            // Replace the line below with the correct import once verified.
            let mut processor = alacritty_terminal::ansi::Processor::new();
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let mut term = term_reader.lock().unwrap();
                        for &byte in &buf[..n] {
                            processor.advance(&mut *term, byte);
                        }
                        dirty_reader.store(true, Ordering::Release);
                    }
                }
            }
        })?;

    Ok(Pane::new(id, term, Box::new(pty_writer), event_rx, dirty, cols, rows))
}

pub fn find_pane<'a>(panes: &'a mut Vec<Pane>, id: synapse_ui::pane::PaneId) -> Option<&'a mut Pane> {
    panes.iter_mut().find(|p| p.id == id)
}
```

- [ ] **Step 3: Fix Processor import**

If `alacritty_terminal::ansi::Processor` doesn't exist, check:
```bash
grep -r "struct Processor" target/doc/alacritty_terminal/ 2>/dev/null | head -5
```

Update the import to the correct path.

- [ ] **Step 4: Compile check**

```bash
cargo build -p SYNAPSE_-app 2>&1 | head -40
```

Fix any compile errors related to the new Pane API.

- [ ] **Step 5: Commit**

```bash
git add crates/SYNAPSE_-app/src/pane_ops.rs
git commit -m "feat(app): rewrite create_pane() using alacritty_terminal + portable-pty"
```

---

## Task 8: Update render.rs — read grid from alacritty_terminal

**Files:**
- Modify: `crates/SYNAPSE_-app/src/render.rs`

- [ ] **Step 1: Add color conversion helper**

Add to the top of render.rs (after existing imports):

```rust
use alacritty_terminal::vte::ansi::Color as TermColor;

fn term_color_to_rgba(color: TermColor, fallback: [f32; 4]) -> [f32; 4] {
    match color {
        TermColor::Spec(rgb) => {
            [rgb.r as f32 / 255.0, rgb.g as f32 / 255.0, rgb.b as f32 / 255.0, 1.0]
        }
        TermColor::Named(_) => fallback,
        TermColor::Indexed(idx) => xterm256_to_rgba(idx),
    }
}

fn xterm256_to_rgba(idx: u8) -> [f32; 4] {
    // Standard xterm 256-color palette
    let rgb: [u8; 3] = match idx {
        // System colors (0-7)
        0 => [0, 0, 0],
        1 => [128, 0, 0],
        2 => [0, 128, 0],
        3 => [128, 128, 0],
        4 => [0, 0, 128],
        5 => [128, 0, 128],
        6 => [0, 128, 128],
        7 => [192, 192, 192],
        // Bright system colors (8-15)
        8 => [128, 128, 128],
        9 => [255, 0, 0],
        10 => [0, 255, 0],
        11 => [255, 255, 0],
        12 => [0, 0, 255],
        13 => [255, 0, 255],
        14 => [0, 255, 255],
        15 => [255, 255, 255],
        // 6x6x6 color cube (16-231)
        16..=231 => {
            let n = idx - 16;
            let b = (n % 6) * 51;
            let g = ((n / 6) % 6) * 51;
            let r = (n / 36) * 51;
            [r, g, b]
        }
        // Grayscale ramp (232-255)
        232..=255 => {
            let v = (idx - 232) * 10 + 8;
            [v, v, v]
        }
    };
    [rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0, 1.0]
}
```

- [ ] **Step 2: Replace the grid-reading section in build_cell_data()**

Find the section in `render.rs` that iterates `pane.grid.borrow().visible_cells_bounded()` (or similar). Replace the grid read block with:

```rust
// Lock the terminal and read visible cells
let term = pane.term.lock().unwrap();
let grid = term.grid();
let cursor_point = term.grid().cursor.point;

for indexed in grid.display_iter() {
    let col = indexed.point.column.0 as f32;
    let row = indexed.point.line.0 as f32;

    let x = rect.x + margin + col * cell_w;
    let y = rect.y + margin + row * cell_h;

    let ch = indexed.c;
    let fg = term_color_to_rgba(indexed.fg, theme.fg);
    let bg = term_color_to_rgba(indexed.bg, theme.bg);

    // Cursor highlight
    let (final_fg, final_bg) = if indexed.point == cursor_point && cursor_visible {
        (theme.bg, theme.cursor)
    } else {
        (fg, bg)
    };

    if ch != ' ' || final_bg != theme.bg {
        cells.push((ch, x, y, font_size, final_fg, final_bg));
    }
}
```

- [ ] **Step 3: Update dirty check**

Find where the code checks `pane.grid.borrow().is_dirty()` (or similar dirty detection). Replace with:

```rust
let is_dirty = pane.is_dirty();
```

- [ ] **Step 4: Update PTY write calls**

Find all `pane.pty_session.pty.write(...)` calls. Replace with:

```rust
pane.write_to_pty(data);
```

- [ ] **Step 5: Update resize calls in app.rs**

In `app.rs`, find `pane.grid.borrow_mut().resize(new_cols, new_rows)`. Replace with:

```rust
{
    let mut term = pane.term.lock().unwrap();
    let new_size = synapse_app::TermSize { cols: new_cols, rows: new_rows };
    term.resize(new_size);
}
```

Note: `term.resize()` may have a different signature. Check cargo doc and adjust. Also update the PTY resize call:

```rust
// Keep this call — pty_writer doesn't handle resize
// PTY resize uses the MasterPty handle, which we need to retain in Pane.
```

**Important:** Add `pty_master: Arc<Mutex<Box<dyn MasterPty + Send>>>` to the `Pane` struct if you need to call `resize()` on the PTY. Update `create_pane()` accordingly:

```rust
// In create_pane(), before taking the writer:
let pty_master_for_resize = pty_pair.master.try_clone()?; // if MasterPty is Clone
// Otherwise store the Arc before taking writer
```

Check if `portable_pty::MasterPty` supports `resize(&self, size: PtySize)` — it does. Store it separately if needed.

- [ ] **Step 6: Update focus event mode check in app.rs**

Find `pane.modes.borrow().focus_events`. Replace with:

```rust
// Check if FOCUS_IN_OUT mode is set in alacritty_terminal
// The mode flag name may be TermMode::FOCUS_IN_OUT or similar — verify in cargo doc
use alacritty_terminal::term::TermMode;
let send_focus = {
    let term = pane.term.lock().unwrap();
    term.mode().contains(TermMode::FOCUS_IN_OUT)
};
if send_focus {
    let seq: &[u8] = if focused { b"\x1b[I" } else { b"\x1b[O" };
    pane.write_to_pty(seq);
}
```

- [ ] **Step 7: Attempt full workspace build**

```bash
cargo build -p SYNAPSE_-app 2>&1 | head -60
```

Fix each compile error. Common issues:
- `pane.modes` references → remove or replace with `term.mode()`
- `pane.processor` references → remove (processing now happens in reader thread)
- `pane.pty_session` references → replace with `pane.write_to_pty()` or `pane.pty_master`
- Any remaining `synapse_terminal::` imports → remove

- [ ] **Step 8: Commit**

```bash
git add crates/SYNAPSE_-app/src/render.rs crates/SYNAPSE_-app/src/app.rs
git commit -m "feat(app): read grid from alacritty_terminal, update PTY write paths"
```

---

## Task 9: Full build and compile error resolution

**Files:** Various (fix-as-you-go)

- [ ] **Step 1: Full workspace build**

```bash
cargo build --workspace 2>&1
```

- [ ] **Step 2: Fix remaining synapse_terminal references**

```bash
grep -r "synapse_terminal\|SYNAPSE_-terminal\|synapse_terminal" crates/ --include="*.rs" -l
```

For each file found: remove or replace the import with the alacritty_terminal equivalent.

- [ ] **Step 3: Fix remaining cosmic_text references**

```bash
grep -r "cosmic_text\|cosmic-text\|CacheKey\|SwashImage\|SwashCache\|FontSystem" crates/ --include="*.rs" -l
```

Remove all cosmic_text imports and update call sites.

- [ ] **Step 4: Run clippy**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | head -60
```

Fix all warnings (they are treated as errors).

- [ ] **Step 5: Run tests**

```bash
cargo test --workspace 2>&1
```

Expected: all tests pass.

- [ ] **Step 6: Commit fixes**

```bash
git add -p  # stage only relevant changes
git commit -m "fix: resolve all compilation errors after alacritty_terminal migration"
```

---

## Task 10: Smoke test — run SYNAPSE_

- [ ] **Step 1: Build release**

```bash
cargo build -p SYNAPSE_-app 2>&1 | tail -5
```

Expected: `Finished dev` (no errors).

- [ ] **Step 2: Run SYNAPSE_**

```bash
cargo run -p SYNAPSE_-app 2>&1
```

Expected:
- Window opens
- Terminal renders (no blank window)
- Shell prompt appears
- Typing produces correct characters
- No glyph rendering artifacts (the original bug)

- [ ] **Step 3: Basic smoke checks**

In the running terminal, type:
```
echo "Hello, SYNAPSE_ v0.2!"
ls -la
vim  # should open, press :q to quit
htop  # verify color rendering, press q to quit
```

Verify:
- ASCII renders without artifacts
- Colors work (file type colors in ls, htop colors)
- Input is responsive

- [ ] **Step 4: Commit if clean**

```bash
git add -A
git commit -m "feat: SYNAPSE_ v2 Phase 1 complete — alacritty_terminal + fontdue foundation"
```

---

## Task 11: Unit tests for terminal integration

**Files:**
- Modify: `crates/SYNAPSE_-ui/src/pane.rs` (add inline test)

- [ ] **Step 1: Add pane unit tests**

Add to the end of `crates/SYNAPSE_-ui/src/pane.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_id_ordering() {
        let a = PaneId(1);
        let b = PaneId(2);
        assert!(a < b);
        assert_eq!(a, PaneId(1));
    }

    #[test]
    fn dirty_flag_clears_on_read() {
        use std::sync::atomic::Ordering;
        let dirty = Arc::new(AtomicBool::new(true));
        // Simulating is_dirty() behavior
        let was_dirty = dirty.swap(false, Ordering::AcqRel);
        assert!(was_dirty);
        let now_dirty = dirty.load(Ordering::Acquire);
        assert!(!now_dirty);
    }
}
```

- [ ] **Step 2: Run all tests**

```bash
cargo test --workspace 2>&1
```

Expected: all pass.

- [ ] **Step 3: Commit**

```bash
git add crates/SYNAPSE_-ui/src/pane.rs
git commit -m "test: add pane unit tests for PaneId and dirty flag"
```

---

## Phase 1 Complete

At this point SYNAPSE_ v2 Phase 1 is done:

- [x] `SYNAPSE_-terminal` crate removed
- [x] `alacritty_terminal` handles all VT parsing, grid, scrollback
- [x] `fontdue` rasterizes glyphs correctly (no artifacts)
- [x] PTY reader thread feeds bytes into `Term` via `Processor`
- [x] `dirty` flag drives frame rebuild
- [x] Shaders (cell.wgsl, ui.wgsl) untouched
- [x] All tests pass

**Next:** Phase 2 plan covers UI rework (tabs, splits cleanup, config). Phase 3 covers `SYNAPSE_-suggest` autosuggestions.
