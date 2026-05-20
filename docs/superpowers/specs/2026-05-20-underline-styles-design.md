# Underline Styles — Design Spec

**Date:** 2026-05-20
**Scope:** All 5 VT underline variants + per-cell color (SGR 58)
**Target:** Neovim diagnostics, spell check, LSP decorations

---

## Problem

`render.rs` never reads `cell.flags` underline bits. All 5 variants
(`UNDERLINE`, `DOUBLE_UNDERLINE`, `UNDERCURL`, `DOTTED_UNDERLINE`,
`DASHED_UNDERLINE`) and per-cell underline color (`CellExtra::underline_color`)
are silently ignored. Neovim users see no diagnostic decorations.

---

## Architecture

New `UnderlineRenderer` — separate wgpu pipeline, does not touch existing
`UIRenderer` or `UIRect`. Avoids regression risk on cursor/borders/search bar.

### Files

| Action | Path |
|--------|------|
| NEW | `crates/SYNAPSE_-renderer/src/underline.rs` |
| NEW | `crates/SYNAPSE_-renderer/src/underline.wgsl` |
| MOD | `crates/SYNAPSE_-renderer/src/renderer.rs` |
| MOD | `crates/SYNAPSE_-renderer/src/lib.rs` |
| MOD | `crates/SYNAPSE_-app/src/render.rs` |

### Draw Order

1. `draw_bg_rects` — cell backgrounds
2. `draw_cells` — text glyphs
3. `draw_underlines` ← new, between text and overlays
4. `draw_ui_rects` — cursor, borders, search bar
5. `draw_images` — Kitty protocol images

---

## Data Structures

```rust
// crates/SYNAPSE_-renderer/src/underline.rs
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UnderlineInstance {
    pub pos:   [f32; 2],  // x, y top-left of underline rect
    pub size:  [f32; 2],  // width, height
    pub color: [f32; 4],  // RGBA linear
    pub style: u32,       // 0=line 1=double 2=curl 3=dot 4=dash
    pub _pad:  [u32; 3],  // pad to 48 bytes
}
```

### Y Position and Height per Style

| Style | `u32` | Y from cell top | Height |
|-------|-------|-----------------|--------|
| line  | 0 | `cell_h - 2.0` | `1.5` |
| double | 1 | `cell_h - 3.5` | `3.0` |
| curl  | 2 | `cell_h - 4.0` | `4.0` |
| dot   | 3 | `cell_h - 2.0` | `1.5` |
| dash  | 4 | `cell_h - 2.0` | `1.5` |

---

## Span Merging (`render.rs`)

Consecutive cells on same row with same style + same color → one
`UnderlineInstance`. Break on: style change, color change, non-underlined
cell (space or flag clear), column gap.

```rust
// Priority order for overlapping flags
let style = if flags.contains(Flags::UNDERCURL)        { Some(2u32) }
       else if flags.contains(Flags::DOUBLE_UNDERLINE) { Some(1) }
       else if flags.contains(Flags::DOTTED_UNDERLINE) { Some(3) }
       else if flags.contains(Flags::DASHED_UNDERLINE) { Some(4) }
       else if flags.contains(Flags::UNDERLINE)        { Some(0) }
       else                                            { None };

let ul_color = cell.underline_color()
    .map(|c| convert_color(c, &state.theme))
    .unwrap_or(fg_color);  // fallback: cell fg
```

---

## Shader (`underline.wgsl`)

Vertex: 6-vertex quad per instance, UV `[0..1, 0..1]`.

Fragment dispatch by `style`:

```wgsl
// style 0 — line
out = color;

// style 1 — double: two bands
if (uv.y < 0.35 || uv.y > 0.65) { out = color; } else { discard; }

// style 2 — curl: sine wave
let freq = 2.5;
let wave = sin(uv.x * freq * 6.2832);
let band_center = wave * 0.3 + 0.5;
if (abs(uv.y - band_center) < 0.18) { out = color; } else { discard; }

// style 3 — dot: circular dots
// dx,dy both normalized to period: range [-0.5, 0.5], radius 0.2 = 40% of period
let dot_period = 0.25;
let dx = fract(uv.x / dot_period) - 0.5;
let dy = uv.y - 0.5;
if (sqrt(dx*dx + dy*dy) < 0.2) { out = color; } else { discard; }

// style 4 — dash
if (fract(uv.x * 4.0) < 0.55) { out = color; } else { discard; }
```

All style constants are shader literals — no uniforms needed.

---

## UnderlineRenderer API

```rust
impl UnderlineRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self;
    pub fn upload(&mut self, instances: &[UnderlineInstance], queue: &wgpu::Queue);
    pub fn draw<'rp>(&'rp self, pass: &mut wgpu::RenderPass<'rp>);
}
```

Bind groups: only screen uniform (vec2 screen size, shared with other renderers).

---

## Out of Scope

- Underline thickness config (fixed per style)
- Animated undercurl (static wave, no time uniform)
- Sixel/iTerm2 image protocol
- Regex search
