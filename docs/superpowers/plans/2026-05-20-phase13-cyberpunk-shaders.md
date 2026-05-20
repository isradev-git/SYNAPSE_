# Phase 13 — Cyberpunk Shaders (Postproc Pipeline) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a GPU postprocessing pipeline to SYNAPSE_ that applies configurable cyberpunk visual effects (CRT scanlines, bloom/glow, chromatic aberration, glitch, matrix rain background, hex grid background, neon pane border pulse, cursor trail) with zero extra overhead when effects are disabled.

**Architecture:** An intermediate offscreen texture receives all existing renderer output unchanged. A `PostProcRenderer` then reads this texture through two passes — a bloom horizontal-blur pass (skipped when bloom is off) and a final composite pass that applies all shader effects before writing to the display surface. Non-shader effects (neon border pulse, cursor trail) live in the app layer using the existing `UIRenderer`. All effects are toggled by a bitmask uniform and configured via `[effects]` in `config.toml`.

**Tech Stack:** wgpu 22, WGSL shaders, bytemuck 1 (already in workspace), serde (existing), winit 0.30, Rust workspace with 5 crates.

---

## File Map

**Create:**
- `crates/SYNAPSE_-config/src/effects.rs` — `EffectsConfig` + sub-config structs
- `crates/SYNAPSE_-renderer/src/postproc.rs` — `PostProcRenderer`, `PostProcUniform`, `BloomUniform`, bitmask constants
- `crates/SYNAPSE_-renderer/src/shaders/postproc.wgsl` — fullscreen triangle + all shader effects
- `crates/SYNAPSE_-renderer/src/shaders/bloom_h.wgsl` — bloom threshold + horizontal gaussian pass

**Modify:**
- `crates/SYNAPSE_-config/src/lib.rs` — add `pub mod effects` + re-export `EffectsConfig`
- `crates/SYNAPSE_-config/src/config.rs` — add `effects: EffectsConfig` field
- `crates/SYNAPSE_-config/src/keybinds.rs` — add `EffectsToggle` to `Action` + `from_str` + default binding
- `crates/SYNAPSE_-renderer/src/lib.rs` — add `pub mod postproc`
- `crates/SYNAPSE_-renderer/src/renderer.rs` — add `postproc: PostProcRenderer`, `effects_enabled`, `effects_config`, `start_time`; refactor `render_instances` to use offscreen → postproc chain
- `crates/SYNAPSE_-app/src/state.rs` — add `effects_enabled: bool` runtime flag
- `crates/SYNAPSE_-app/src/app.rs` — handle `Action::EffectsToggle`; call `renderer.set_effects_enabled` + `renderer.set_effects_config`
- `crates/SYNAPSE_-app/src/render.rs` — cursor trail rendering, neon border pulse animation

---

## Background: How the render pipeline works

`Renderer::render_instances` (renderer.rs:498) currently:
1. `surface.get_current_texture()` → gets the display surface texture
2. Creates `view` from the surface texture
3. Runs a single `RenderPass` that clears → draws bg/image/glyph/underline/overlay layers
4. `queue.submit` + `output.present()`

After this change:
1. Same surface texture acquisition
2. Main pass renders to **`postproc.offscreen_view`** (NOT the surface)
3. `PostProcRenderer::render` runs: optionally bloom H pass → final composite to surface
4. Same `queue.submit` + `output.present`

The external `draw_frame_with_options` signature is **unchanged**. Effects state is stored inside `Renderer` and updated via `set_effects_enabled` / `set_effects_config`.

---

## Task 1: EffectsConfig struct in SYNAPSE_-config

**Files:**
- Create: `crates/SYNAPSE_-config/src/effects.rs`
- Modify: `crates/SYNAPSE_-config/src/lib.rs`
- Modify: `crates/SYNAPSE_-config/src/config.rs`
- Test: `crates/SYNAPSE_-config/src/effects.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Write the failing test**

Add to `crates/SYNAPSE_-config/src/effects.rs` (create file):

```rust
use serde::{Deserialize, Serialize};

fn default_scanline_intensity() -> f32 { 0.3 }
fn default_scanline_freq() -> f32 { 2.0 }
fn default_bloom_threshold() -> f32 { 0.7 }
fn default_bloom_sigma() -> f32 { 4.0 }
fn default_bloom_tint() -> String { "#FF003C".to_string() }
fn default_chroma_strength() -> f32 { 0.002 }
fn default_matrix_color() -> String { "#00FF55".to_string() }
fn default_matrix_density() -> f32 { 0.3 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScanlinesConfig {
    #[serde(default = "default_scanline_intensity")]
    pub intensity: f32,
    #[serde(default = "default_scanline_freq")]
    pub freq: f32,
}

impl Default for ScanlinesConfig {
    fn default() -> Self {
        Self { intensity: default_scanline_intensity(), freq: default_scanline_freq() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BloomConfig {
    #[serde(default = "default_bloom_threshold")]
    pub threshold: f32,
    #[serde(default = "default_bloom_sigma")]
    pub sigma: f32,
    #[serde(default = "default_bloom_tint")]
    pub tint: String,
}

impl Default for BloomConfig {
    fn default() -> Self {
        Self {
            threshold: default_bloom_threshold(),
            sigma: default_bloom_sigma(),
            tint: default_bloom_tint(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChromaConfig {
    #[serde(default = "default_chroma_strength")]
    pub strength: f32,
}

impl Default for ChromaConfig {
    fn default() -> Self { Self { strength: default_chroma_strength() } }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatrixBgConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_matrix_color")]
    pub color: String,
    #[serde(default = "default_matrix_density")]
    pub density: f32,
}

impl Default for MatrixBgConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            color: default_matrix_color(),
            density: default_matrix_density(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct EffectsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub scanlines: ScanlinesConfig,
    #[serde(default)]
    pub bloom: BloomConfig,
    #[serde(default)]
    pub chroma: ChromaConfig,
    #[serde(default)]
    pub matrix_bg: MatrixBgConfig,
    #[serde(default)]
    pub hex_grid: bool,
    #[serde(default)]
    pub pane_pulse: bool,
    #[serde(default)]
    pub cursor_trail: u8,
}

/// Parse a hex color string "#RRGGBB" into [r, g, b, 1.0] floats.
/// Returns cyan [0,1,1,1] as fallback for invalid input.
pub fn parse_hex_color(s: &str) -> [f32; 4] {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return [0.0, 1.0, 1.0, 1.0];
    }
    let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(0) as f32 / 255.0;
    let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(255) as f32 / 255.0;
    let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(255) as f32 / 255.0;
    [r, g, b, 1.0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effects_config_defaults() {
        let cfg = EffectsConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.scanlines.intensity, 0.3);
        assert_eq!(cfg.scanlines.freq, 2.0);
        assert_eq!(cfg.bloom.threshold, 0.7);
        assert_eq!(cfg.bloom.sigma, 4.0);
        assert_eq!(cfg.bloom.tint, "#FF003C");
        assert_eq!(cfg.chroma.strength, 0.002);
        assert!(!cfg.matrix_bg.enabled);
        assert_eq!(cfg.matrix_bg.density, 0.3);
        assert!(!cfg.hex_grid);
        assert!(!cfg.pane_pulse);
        assert_eq!(cfg.cursor_trail, 0);
    }

    #[test]
    fn test_effects_config_toml_round_trip() {
        let cfg = EffectsConfig {
            enabled: true,
            scanlines: ScanlinesConfig { intensity: 0.5, freq: 3.0 },
            cursor_trail: 4,
            pane_pulse: true,
            ..EffectsConfig::default()
        };
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let parsed: EffectsConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn test_effects_partial_toml() {
        let toml_str = r#"
enabled = true
cursor_trail = 3
pane_pulse = true
"#;
        let cfg: EffectsConfig = toml::from_str(toml_str).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.cursor_trail, 3);
        assert!(cfg.pane_pulse);
        // Defaults still apply
        assert_eq!(cfg.scanlines.intensity, 0.3);
        assert_eq!(cfg.bloom.threshold, 0.7);
    }

    #[test]
    fn test_parse_hex_color() {
        let c = parse_hex_color("#FF003C");
        assert!((c[0] - 1.0).abs() < 0.01);
        assert!(c[1] < 0.01);
        assert!((c[2] - 0.235).abs() < 0.01);
        assert_eq!(c[3], 1.0);

        let c = parse_hex_color("#00FF55");
        assert!(c[0] < 0.01);
        assert!((c[1] - 1.0).abs() < 0.01);
        assert!((c[2] - 0.333).abs() < 0.01);

        // Invalid → cyan fallback
        let c = parse_hex_color("notacolor");
        assert_eq!(c, [0.0, 1.0, 1.0, 1.0]);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
~/.cargo/bin/cargo test -p SYNAPSE_-config effects -- --nocapture
```

Expected: `FAIL` — module `effects` not found.

- [ ] **Step 3: Wire effects module into lib.rs and Config**

Add to `crates/SYNAPSE_-config/src/lib.rs`:
```rust
pub mod effects;
pub use effects::EffectsConfig;
```

Add to `crates/SYNAPSE_-config/src/config.rs` after the existing `use` statements:
```rust
use crate::effects::EffectsConfig;
```

Add field to `Config` struct (after `theme: String`):
```rust
    #[serde(default)]
    pub effects: EffectsConfig,
```

Add to `Config::default()` impl:
```rust
            effects: EffectsConfig::default(),
```

Also add `effects: EffectsConfig::default()` to all Config construction sites in existing tests (none needed — they use `..Config::default()` spread).

- [ ] **Step 4: Run tests**

```bash
~/.cargo/bin/cargo test -p SYNAPSE_-config -- --nocapture
```

Expected: all existing + new tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/SYNAPSE_-config/src/effects.rs crates/SYNAPSE_-config/src/lib.rs crates/SYNAPSE_-config/src/config.rs
git commit -m "feat(config): add EffectsConfig for postproc shader settings"
```

---

## Task 2: Action::EffectsToggle + default keybinding

**Files:**
- Modify: `crates/SYNAPSE_-config/src/keybinds.rs`

- [ ] **Step 1: Write the failing test**

Locate the `#[cfg(test)]` block at the bottom of `keybinds.rs` and add:

```rust
    #[test]
    fn test_effects_toggle_action() {
        assert_eq!(Action::from_str("effects_toggle"), Some(Action::EffectsToggle));
        assert_eq!(Action::from_str("unknown_xyz"), None);
    }

    #[test]
    fn test_effects_toggle_default_binding() {
        let kb = Keybinds::default();
        let has_effects = kb.bindings().any(|(_, a)| *a == Action::EffectsToggle);
        assert!(has_effects, "EffectsToggle must have a default binding");
    }
```

- [ ] **Step 2: Run test to verify it fails**

```bash
~/.cargo/bin/cargo test -p SYNAPSE_-config test_effects_toggle -- --nocapture
```

Expected: `FAIL` — `Action::EffectsToggle` does not exist.

- [ ] **Step 3: Add EffectsToggle to Action enum**

In `Action` enum, add after `ReloadConfig`:
```rust
    EffectsToggle,
```

In `Action::from_str`, add before the `_ => None` arm:
```rust
            "effects_toggle" => Some(Action::EffectsToggle),
```

- [ ] **Step 4: Add default binding in Keybinds::default()**

Find where `Keybinds::default()` is defined (around line 130+). Add the default binding for Ctrl+Shift+E:

```rust
        // effects toggle
        bindings.insert(
            KeyCombo { key: "e".to_string(), ctrl: true, shift: true, alt: false },
            Action::EffectsToggle,
        );
```

Also add `EffectsToggle` to whatever `bindings()` iterator or display impl exists so it appears in config docs.

- [ ] **Step 5: Run tests**

```bash
~/.cargo/bin/cargo test -p SYNAPSE_-config -- --nocapture
```

Expected: all pass including new tests.

- [ ] **Step 6: Commit**

```bash
git add crates/SYNAPSE_-config/src/keybinds.rs
git commit -m "feat(config): add Action::EffectsToggle with Ctrl+Shift+E default binding"
```

---

## Task 3: PostProcRenderer — offscreen texture + passthrough pipeline

**Files:**
- Create: `crates/SYNAPSE_-renderer/src/postproc.rs`
- Create: `crates/SYNAPSE_-renderer/src/shaders/postproc.wgsl` (passthrough only)
- Create: `crates/SYNAPSE_-renderer/src/shaders/bloom_h.wgsl` (passthrough bloom, returns black)
- Modify: `crates/SYNAPSE_-renderer/src/lib.rs`

- [ ] **Step 1: Create bloom_h.wgsl (stub — returns black)**

```wgsl
// bloom_h.wgsl — horizontal bloom pass (threshold + horizontal gaussian blur)
// Returns bright pixels blurred horizontally; black for sub-threshold pixels.

struct BloomUniform {
    screen_size: vec2<f32>,
    threshold: f32,
    sigma: f32,
}

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(1) @binding(0) var<uniform> u: BloomUniform;

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    var x = -1.0;
    var y = -1.0;
    if vid == 1u { x = 3.0; }
    if vid == 2u { y = 3.0; }
    return vec4(x, y, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) frag_pos: vec4<f32>) -> @location(0) vec4<f32> {
    // Stub: return black until bloom is implemented in Task 6
    return vec4(0.0, 0.0, 0.0, 1.0);
}
```

- [ ] **Step 2: Create postproc.wgsl (passthrough)**

```wgsl
// postproc.wgsl — fullscreen composite pass
// Applies all enabled cyberpunk effects then writes to surface.

// Effect bitmask constants (must match Rust EFFECT_* constants in postproc.rs)
const EFFECT_SCANLINES: u32 = 1u;
const EFFECT_BLOOM:     u32 = 2u;
const EFFECT_CHROMA:    u32 = 4u;
const EFFECT_GLITCH:    u32 = 8u;
const EFFECT_MATRIX_BG: u32 = 16u;
const EFFECT_HEX_GRID:  u32 = 32u;

struct PostProcUniform {
    screen_size:      vec2<f32>,  // offset  0
    time:             f32,         // offset  8
    effects_mask:     u32,         // offset 12
    scanline_intensity: f32,       // offset 16
    scanline_freq:    f32,         // offset 20
    bloom_threshold:  f32,         // offset 24
    bloom_sigma:      f32,         // offset 28
    bloom_tint:       vec4<f32>,   // offset 32 (16-byte aligned)
    chroma_strength:  f32,         // offset 48
    glitch_intensity: f32,         // offset 52
    matrix_density:   f32,         // offset 56
    _pad:             f32,         // offset 60
    matrix_color:     vec4<f32>,   // offset 64 (16-byte aligned)
    // SizeOf = 80 bytes
}

@group(0) @binding(0) var scene_tex:    texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var bloom_tex:    texture_2d<f32>;
@group(1) @binding(0) var<uniform> u: PostProcUniform;

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    // Fullscreen triangle: 3 vertices cover the entire screen
    var x = -1.0;
    var y = -1.0;
    if vid == 1u { x = 3.0; }
    if vid == 2u { y = 3.0; }
    return vec4(x, y, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) frag_pos: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = frag_pos.xy / u.screen_size;
    // Passthrough — effects added in Tasks 5-8
    let color = textureSample(scene_tex, scene_sampler, uv).rgb;
    return vec4(color, 1.0);
}
```

- [ ] **Step 3: Create postproc.rs**

```rust
use std::sync::Arc;
use wgpu::{BindGroup, Buffer, Device, Queue, RenderPipeline, Texture, TextureView};

// Bitmask constants — must match WGSL constants in postproc.wgsl
pub const EFFECT_SCANLINES: u32 = 1 << 0;
pub const EFFECT_BLOOM:     u32 = 1 << 1;
pub const EFFECT_CHROMA:    u32 = 1 << 2;
pub const EFFECT_GLITCH:    u32 = 1 << 3;
pub const EFFECT_MATRIX_BG: u32 = 1 << 4;
pub const EFFECT_HEX_GRID:  u32 = 1 << 5;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PostProcUniform {
    pub screen_size:       [f32; 2],  // offset  0
    pub time:              f32,        // offset  8
    pub effects_mask:      u32,        // offset 12
    pub scanline_intensity: f32,       // offset 16
    pub scanline_freq:     f32,        // offset 20
    pub bloom_threshold:   f32,        // offset 24
    pub bloom_sigma:       f32,        // offset 28
    pub bloom_tint:        [f32; 4],   // offset 32
    pub chroma_strength:   f32,        // offset 48
    pub glitch_intensity:  f32,        // offset 52
    pub matrix_density:    f32,        // offset 56
    pub _pad:              f32,        // offset 60
    pub matrix_color:      [f32; 4],   // offset 64
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
    // Offscreen: where the main scene renders to
    offscreen_tex:  Texture,
    offscreen_view: TextureView,
    // Bloom horizontal pass output
    bloom_h_tex:  Texture,
    bloom_h_view: TextureView,
    // Bloom H pass pipeline + resources
    bloom_h_pipeline:    RenderPipeline,
    bloom_h_scene_bg:    BindGroup,  // binds offscreen_tex
    bloom_h_uniform_buf: Buffer,
    bloom_h_uniform_bg:  BindGroup,
    // Final composite pipeline + resources
    main_pipeline:    RenderPipeline,
    main_scene_bg:    BindGroup,  // binds offscreen_tex + bloom_h_tex
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
            label:       Some("SYNAPSE_ PostProc Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter:  wgpu::FilterMode::Linear,
            min_filter:  wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // ── Bloom H pass ──────────────────────────────────────────────────────

        let bloom_h_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("SYNAPSE_ BloomH Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/bloom_h.wgsl").into(),
            ),
        });

        let bloom_h_tex_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:   Some("SYNAPSE_ BloomH Tex BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type:    wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled:   false,
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

        let bloom_h_uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:   Some("SYNAPSE_ BloomH Uniform BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
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
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&offscreen_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });

        let bloom_h_uniform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("SYNAPSE_ BloomH Uniform BG"),
            layout:  &bloom_h_uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
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
                module:      &bloom_h_shader,
                entry_point: "vs_main",
                buffers:     &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:      &bloom_h_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend:       Some(wgpu::BlendState::REPLACE),
                    write_mask:  wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive:    wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample:  wgpu::MultisampleState::default(),
            multiview:    None,
            cache:        None,
        });

        // ── Main composite pass ───────────────────────────────────────────────

        let main_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("SYNAPSE_ PostProc Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/postproc.wgsl").into(),
            ),
        });

        let main_tex_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:   Some("SYNAPSE_ PostProc Tex BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type:    wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled:   false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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
                binding: 0,
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
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&offscreen_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&bloom_h_view) },
            ],
        });

        let main_uniform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("SYNAPSE_ PostProc Uniform BG"),
            layout:  &main_uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
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
                module:      &main_shader,
                entry_point: "vs_main",
                buffers:     &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:      &main_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend:       Some(wgpu::BlendState::REPLACE),
                    write_mask:  wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive:    wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample:  wgpu::MultisampleState::default(),
            multiview:    None,
            cache:        None,
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
            label:              Some(label),
            size:               wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count:    1,
            sample_count:       1,
            dimension:          wgpu::TextureDimension::D2,
            format,
            usage:              wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats:       &[],
        })
    }

    /// Recreate offscreen + bloom textures after a window resize.
    pub fn resize(&mut self, width: u32, height: u32, queue: &Queue) {
        self.offscreen_tex  = Self::make_tex(&self.device, self.format, width, height, "SYNAPSE_ Offscreen");
        self.offscreen_view = self.offscreen_tex.create_view(&Default::default());
        self.bloom_h_tex    = Self::make_tex(&self.device, self.format, width, height, "SYNAPSE_ BloomH");
        self.bloom_h_view   = self.bloom_h_tex.create_view(&Default::default());

        // Recreate bind groups with new texture views
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter:     wgpu::FilterMode::Linear,
            min_filter:     wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Rebuild bloom_h scene bind group
        let bloom_h_tex_bgl = self.bloom_h_pipeline.get_bind_group_layout(0);
        self.bloom_h_scene_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("SYNAPSE_ BloomH Scene BG"),
            layout:  &bloom_h_tex_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.offscreen_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });

        // Rebuild main scene bind group
        let main_tex_bgl = self.main_pipeline.get_bind_group_layout(0);
        self.main_scene_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("SYNAPSE_ PostProc Scene BG"),
            layout:  &main_tex_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.offscreen_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.bloom_h_view) },
            ],
        });

        let _ = queue; // reserved for future init uploads
    }

    /// Returns the offscreen texture view — the main render pass writes here.
    pub fn offscreen_view(&self) -> &TextureView {
        &self.offscreen_view
    }

    /// Run bloom H pass (if bloom enabled) then final composite to `surface_view`.
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
```

- [ ] **Step 4: Wire into lib.rs**

Add to `crates/SYNAPSE_-renderer/src/lib.rs`:
```rust
pub mod postproc;
```

- [ ] **Step 5: Build + run unit tests**

```bash
~/.cargo/bin/cargo test -p SYNAPSE_-renderer postproc -- --nocapture
```

Expected: all 3 unit tests pass (`test_postproc_uniform_size`, `test_bloom_uniform_size`, `test_effect_bitmasks_unique`).

```bash
~/.cargo/bin/cargo build -p SYNAPSE_-renderer
```

Expected: compiles clean.

- [ ] **Step 6: Commit**

```bash
git add crates/SYNAPSE_-renderer/src/postproc.rs \
        crates/SYNAPSE_-renderer/src/lib.rs \
        crates/SYNAPSE_-renderer/src/shaders/postproc.wgsl \
        crates/SYNAPSE_-renderer/src/shaders/bloom_h.wgsl
git commit -m "feat(renderer): add PostProcRenderer with offscreen texture + passthrough pipeline"
```

---

## Task 4: Wire postproc into Renderer + app layer

**Files:**
- Modify: `crates/SYNAPSE_-renderer/src/renderer.rs`
- Modify: `crates/SYNAPSE_-app/src/state.rs`
- Modify: `crates/SYNAPSE_-app/src/app.rs`

- [ ] **Step 1: Add fields to Renderer**

Add to `use` imports at top of `renderer.rs`:
```rust
use std::time::Instant;
use synapse_config::EffectsConfig;
use crate::postproc::{PostProcRenderer, PostProcUniform, EFFECT_BLOOM, EFFECT_CHROMA, EFFECT_GLITCH, EFFECT_HEX_GRID, EFFECT_MATRIX_BG, EFFECT_SCANLINES};
```

Add fields to the `Renderer` struct (after `cached_instances`):
```rust
    postproc:       PostProcRenderer,
    effects_enabled: bool,
    effects_config:  EffectsConfig,
    start_time:      Instant,
```

- [ ] **Step 2: Initialize new fields in Renderer::new**

After `let text = TextShaping::with_family(font_family);` and before `Ok(Self { ... })`:
```rust
        let postproc = PostProcRenderer::new(
            Arc::clone(&device),
            format,
            size.width,
            size.height,
        );
```

In the `Ok(Self { ... })` block, add:
```rust
            postproc,
            effects_enabled: false,
            effects_config: EffectsConfig::default(),
            start_time: Instant::now(),
```

- [ ] **Step 3: Add public methods to set effects state**

Add after the existing `remove_image` method:
```rust
    pub fn set_effects_enabled(&mut self, enabled: bool) {
        self.effects_enabled = enabled;
    }

    pub fn set_effects_config(&mut self, config: EffectsConfig) {
        self.effects_config = config;
    }
```

- [ ] **Step 4: Update resize to also resize postproc textures**

In `Renderer::resize`, after `self.surface.configure(&self.device, &self.config);` add:
```rust
            self.postproc.resize(new_size.width, new_size.height, &self.queue);
```

- [ ] **Step 5: Refactor render_instances to use offscreen → postproc chain**

Replace the existing `render_instances` body (the part from `let output = ...` to `output.present()`) with:

```rust
    fn render_instances(&mut self, images: &[ImageInstance], image_ids: &[u32], clip_rects: &[[u32; 4]]) {
        self.cell_renderer
            .update_screen_size(&self.queue, self.size.width, self.size.height);
        self.ui_renderer_bg
            .update_screen_size(&self.queue, self.size.width, self.size.height);
        self.ui_renderer
            .update_screen_size(&self.queue, self.size.width, self.size.height);
        self.image_renderer
            .update_screen_size(&self.queue, self.size.width, self.size.height);
        self.underline_renderer
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

        let surface_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SYNAPSE_ Render Encoder"),
            });

        // Main scene pass → writes to offscreen texture (NOT the surface)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SYNAPSE_ Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view:           self.postproc.offscreen_view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load:  wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes:         None,
                occlusion_query_set:      None,
            });

            self.ui_renderer_bg.draw(&mut render_pass);
            self.image_renderer.draw_images(
                &mut render_pass,
                images,
                image_ids,
                clip_rects,
                &self.queue,
            );
            self.cell_renderer.draw(&mut render_pass, &self.atlas.bind_group);
            self.underline_renderer.draw(&mut render_pass);
            self.ui_renderer.draw(&mut render_pass);
        }

        // Postproc pass → reads offscreen, writes to surface
        let uniform = self.build_postproc_uniform();
        self.postproc.render(&mut encoder, &surface_view, &self.queue, &uniform);

        self.queue.submit(Some(encoder.finish()));
        output.present();
    }
```

- [ ] **Step 6: Add build_postproc_uniform helper**

Add this method to `Renderer`:
```rust
    fn build_postproc_uniform(&self) -> PostProcUniform {
        use synapse_config::effects::parse_hex_color;

        let time = self.start_time.elapsed().as_secs_f32();
        let cfg  = &self.effects_config;

        let mut mask = 0u32;
        if self.effects_enabled && cfg.enabled {
            if cfg.scanlines.intensity > 0.0    { mask |= EFFECT_SCANLINES; }
            if cfg.bloom.threshold < 1.0        { mask |= EFFECT_BLOOM; }
            if cfg.chroma.strength > 0.0        { mask |= EFFECT_CHROMA; }
            if cfg.matrix_bg.enabled            { mask |= EFFECT_MATRIX_BG; }
            if cfg.hex_grid                     { mask |= EFFECT_HEX_GRID; }
        }

        PostProcUniform {
            screen_size:       [self.size.width as f32, self.size.height as f32],
            time,
            effects_mask:      mask,
            scanline_intensity: cfg.scanlines.intensity,
            scanline_freq:     cfg.scanlines.freq,
            bloom_threshold:   cfg.bloom.threshold,
            bloom_sigma:       cfg.bloom.sigma,
            bloom_tint:        parse_hex_color(&cfg.bloom.tint),
            chroma_strength:   cfg.chroma.strength,
            glitch_intensity:  0.0,  // task 7 wires this
            matrix_density:    cfg.matrix_bg.density,
            _pad:              0.0,
            matrix_color:      parse_hex_color(&cfg.matrix_bg.color),
        }
    }
```

Note: `parse_hex_color` is in `synapse_config::effects`. The renderer crate already depends on `synapse_config`? Check `Cargo.toml`. If not, add: in `crates/SYNAPSE_-renderer/Cargo.toml`, add `synapse_config = { path = "../SYNAPSE_-config" }`. Actually, looking at the codebase, effects_config is passed from the app. To avoid a dep cycle we can either: (a) add synapse_config dep to renderer, or (b) have Renderer accept a pre-built `PostProcUniform` from the app. 

**Use option (b)**: change `build_postproc_uniform` to be called from the app and passed into `draw_frame_with_options`. Add a parameter `postproc_uniform: PostProcUniform` to `draw_frame_with_options`.

Actually, the cleanest approach is to keep Renderer self-contained: store `effects_config: EffectsConfig` inside Renderer, and add `synapse_config` as a dep. This is simpler.

Add to `crates/SYNAPSE_-renderer/Cargo.toml`:
```toml
synapse_config = { path = "../SYNAPSE_-config" }
```

Then the `use synapse_config::...` imports work.

- [ ] **Step 7: Update app state**

In `crates/SYNAPSE_-app/src/state.rs`, find the `AppState` struct and add field:
```rust
    pub effects_enabled: bool,
```

In `AppState::new` (or wherever it's initialized), set:
```rust
            effects_enabled: config.effects.enabled,
```

- [ ] **Step 8: Handle EffectsToggle in app.rs**

In `crates/SYNAPSE_-app/src/app.rs`, find where `Action::ReloadConfig` is handled (in the keyboard event handler). Add alongside it:

```rust
                Action::EffectsToggle => {
                    self.state.effects_enabled = !self.state.effects_enabled;
                    self.renderer.set_effects_enabled(self.state.effects_enabled);
                }
```

In `Action::ReloadConfig` handler, after the config is reloaded, add:
```rust
                    self.renderer.set_effects_config(self.state.config.effects.clone());
```

Also in `Renderer::new` (or in AppCore init), set initial effects from config:
```rust
        renderer.set_effects_config(config.effects.clone());
        renderer.set_effects_enabled(config.effects.enabled);
```

- [ ] **Step 9: Build and verify**

```bash
~/.cargo/bin/cargo build -p SYNAPSE_-app
```

Expected: compiles clean. Run the app, confirm terminal still renders normally (postproc passthrough active, effects off).

```bash
~/.cargo/bin/cargo test --workspace -- --nocapture
```

Expected: all tests pass.

- [ ] **Step 10: Commit**

```bash
git add crates/SYNAPSE_-renderer/src/renderer.rs \
        crates/SYNAPSE_-renderer/Cargo.toml \
        crates/SYNAPSE_-app/src/state.rs \
        crates/SYNAPSE_-app/src/app.rs
git commit -m "feat(renderer): wire postproc pipeline — offscreen→composite render path"
```

---

## Task 5: CRT scanlines + vignette + barrel distortion

**Files:**
- Modify: `crates/SYNAPSE_-renderer/src/shaders/postproc.wgsl`

- [ ] **Step 1: Implement barrel distortion UV warp**

Replace the `fs_main` body in `postproc.wgsl` with:

```wgsl
fn hash(n: f32) -> f32 {
    return fract(sin(n) * 43758.5453123);
}

// Barrel distortion: strength ~0.1 gives subtle CRT curve
fn barrel_uv(uv: vec2<f32>, strength: f32) -> vec2<f32> {
    let c = uv * 2.0 - 1.0;
    let r2 = dot(c, c);
    return (c * (1.0 + strength * r2)) * 0.5 + 0.5;
}

fn apply_vignette(uv: vec2<f32>) -> f32 {
    let v = uv * (1.0 - uv.yx);
    return clamp(pow(v.x * v.y * 12.0, 0.35), 0.0, 1.0);
}

fn apply_scanlines(color: vec3<f32>, screen_y: f32, intensity: f32, freq: f32) -> vec3<f32> {
    // Scanline at every `freq` pixels height
    let line = sin(screen_y * 3.14159265 * freq) * 0.5 + 0.5;
    return color * (1.0 - intensity * (1.0 - line));
}

@fragment
fn fs_main(@builtin(position) frag_pos: vec4<f32>) -> @location(0) vec4<f32> {
    var uv = frag_pos.xy / u.screen_size;

    // Barrel distortion (subtle: 0.05)
    if (u.effects_mask & EFFECT_SCANLINES) != 0u {
        uv = barrel_uv(uv, 0.05);
        // clamp to prevent sampling outside texture
        uv = clamp(uv, vec2(0.001), vec2(0.999));
    }

    var color = textureSample(scene_tex, scene_sampler, uv).rgb;

    // CRT scanlines
    if (u.effects_mask & EFFECT_SCANLINES) != 0u {
        color = apply_scanlines(color, frag_pos.y, u.scanline_intensity, u.scanline_freq);
        color = color * apply_vignette(uv);
    }

    return vec4(color, 1.0);
}
```

- [ ] **Step 2: Build to verify shader compiles**

```bash
~/.cargo/bin/cargo build -p SYNAPSE_-renderer
```

Expected: compiles clean (wgpu validates WGSL at pipeline creation time).

- [ ] **Step 3: Manual test**

Add to `~/.config/SYNAPSE_/config.toml`:
```toml
[effects]
enabled = true

[effects.scanlines]
intensity = 0.4
freq = 2.0
```

Run `cargo run -p SYNAPSE_-app`. Expected: visible horizontal scanlines + subtle vignette darkening at edges.

- [ ] **Step 4: Commit**

```bash
git add crates/SYNAPSE_-renderer/src/shaders/postproc.wgsl
git commit -m "feat(shader): CRT scanlines + vignette + barrel distortion"
```

---

## Task 6: Bloom / glow — 2-pass gaussian

**Files:**
- Modify: `crates/SYNAPSE_-renderer/src/shaders/bloom_h.wgsl` (replace stub)
- Modify: `crates/SYNAPSE_-renderer/src/shaders/postproc.wgsl` (add vertical bloom + composite)

- [ ] **Step 1: Implement bloom_h.wgsl (threshold + horizontal gaussian)**

Replace the stub body of `fs_main` in `bloom_h.wgsl`:

```wgsl
@fragment
fn fs_main(@builtin(position) frag_pos: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = frag_pos.xy / u.screen_size;
    let pixel_w = 1.0 / u.screen_size.x;

    // Sample source and check against threshold
    var result = vec3(0.0);
    var weight_sum = 0.0;

    // 13-tap horizontal gaussian, half-sigma in pixel units
    let step_size = pixel_w * max(u.sigma, 1.0);

    for (var i: i32 = -6; i <= 6; i++) {
        let sample_uv = vec2(uv.x + f32(i) * step_size, uv.y);
        let s = textureSample(scene_tex, scene_sampler, clamp(sample_uv, vec2(0.0), vec2(1.0))).rgb;
        let luma = dot(s, vec3(0.2126, 0.7152, 0.0722));
        if luma >= u.threshold {
            let fi = f32(i);
            let w = exp(-0.5 * fi * fi / (u.sigma * u.sigma));
            result += s * w;
            weight_sum += w;
        }
    }

    if weight_sum > 0.0 {
        result /= weight_sum;
    }

    return vec4(result, 1.0);
}
```

- [ ] **Step 2: Add vertical bloom + composite in postproc.wgsl**

Add the following functions before `fs_main`, and update `fs_main` to apply bloom when enabled:

```wgsl
fn sample_bloom_v(uv: vec2<f32>, sigma: f32) -> vec3<f32> {
    let pixel_h = 1.0 / u.screen_size.y;
    let step_size = pixel_h * max(sigma, 1.0);
    var result = vec3(0.0);
    var weight_sum = 0.0;

    for (var i: i32 = -6; i <= 6; i++) {
        let sample_uv = vec2(uv.x, uv.y + f32(i) * step_size);
        let s = textureSample(bloom_tex, scene_sampler, clamp(sample_uv, vec2(0.0), vec2(1.0))).rgb;
        let fi = f32(i);
        let w = exp(-0.5 * fi * fi / (sigma * sigma));
        result += s * w;
        weight_sum += w;
    }

    if weight_sum > 0.0 { result /= weight_sum; }
    return result;
}
```

In `fs_main`, after the scanlines block and before the final return:
```wgsl
    // Bloom: vertical gaussian over bloom_h_tex, add with tint
    if (u.effects_mask & EFFECT_BLOOM) != 0u {
        let bloom_v = sample_bloom_v(uv, u.bloom_sigma);
        let tinted = bloom_v * u.bloom_tint.rgb * 2.0;
        color = color + tinted;
        // Reinhard tonemapping to prevent oversaturation
        color = color / (color + vec3(1.0));
    }
```

- [ ] **Step 3: Build**

```bash
~/.cargo/bin/cargo build -p SYNAPSE_-renderer
```

Expected: clean.

- [ ] **Step 4: Manual test**

Add to config:
```toml
[effects.bloom]
threshold = 0.6
sigma = 3.0
tint = "#FF003C"
```

Run app. Expected: bright glyphs emit a red glow/halo. The effect is subtle at low sigma, dramatic at sigma ≥ 5.

- [ ] **Step 5: Commit**

```bash
git add crates/SYNAPSE_-renderer/src/shaders/bloom_h.wgsl \
        crates/SYNAPSE_-renderer/src/shaders/postproc.wgsl
git commit -m "feat(shader): 2-pass bloom/glow with tint (H-pass + vertical composite)"
```

---

## Task 7: Chromatic aberration + glitch/datamosh

**Files:**
- Modify: `crates/SYNAPSE_-renderer/src/shaders/postproc.wgsl`
- Modify: `crates/SYNAPSE_-renderer/src/renderer.rs` (expose glitch trigger)

- [ ] **Step 1: Add chroma + glitch functions in postproc.wgsl**

Add before `fs_main`:

```wgsl
fn apply_chroma(uv: vec2<f32>, strength: f32) -> vec3<f32> {
    let center = uv - 0.5;
    let dist = length(center);
    let dir = normalize(center + vec2(0.0001));  // avoid zero-divide at center
    let r = textureSample(scene_tex, scene_sampler, clamp(uv + dir * strength * dist, vec2(0.0), vec2(1.0))).r;
    let g = textureSample(scene_tex, scene_sampler, uv).g;
    let b = textureSample(scene_tex, scene_sampler, clamp(uv - dir * strength * dist, vec2(0.0), vec2(1.0))).b;
    return vec3(r, g, b);
}

fn apply_glitch(color: vec3<f32>, uv: vec2<f32>, time: f32, intensity: f32) -> vec3<f32> {
    if intensity <= 0.001 { return color; }
    // Block the screen into horizontal strips; randomly shift some
    let block_y    = floor(uv.y * 24.0);
    let glitch_t   = floor(time * 12.0);
    let noise      = hash(block_y * 13.7 + glitch_t * 0.31);
    if noise > 1.0 - intensity * 0.4 {
        let shift = (hash(block_y * 7.3 + glitch_t) - 0.5) * intensity * 0.08;
        let glitch_uv = vec2(fract(uv.x + shift), uv.y);
        return textureSample(scene_tex, scene_sampler, glitch_uv).rgb;
    }
    return color;
}
```

- [ ] **Step 2: Wire into fs_main**

In `fs_main`, after the bloom block:
```wgsl
    // Chromatic aberration — replaces the simple sample when enabled
    if (u.effects_mask & EFFECT_CHROMA) != 0u {
        color = apply_chroma(uv, u.chroma_strength);
    }

    // Glitch / datamosh
    if u.glitch_intensity > 0.001 {
        color = apply_glitch(color, uv, u.time, u.glitch_intensity);
    }
```

- [ ] **Step 3: Wire glitch trigger in renderer.rs**

Add field to `Renderer`:
```rust
    glitch_timer: f32,  // seconds remaining for glitch burst
```

Initialize as `glitch_timer: 0.0` in `Renderer::new`.

Add public method:
```rust
    pub fn trigger_glitch(&mut self, duration_secs: f32) {
        self.glitch_timer = duration_secs;
    }
```

In `build_postproc_uniform`, compute glitch intensity from timer:
```rust
        // Glitch decays over the timer period
        let dt = self.start_time.elapsed().as_secs_f32();
        // Store last_dt in Renderer to compute delta... simplest: use constant decay per frame
        // For now: glitch_intensity = clamp(glitch_timer, 0, 1)
        let glitch_intensity = self.glitch_timer.clamp(0.0, 1.0);
        // Decay glitch_timer each frame by ~60fps delta
        // (updated in render call — see note below)
```

Actually, to decay `glitch_timer` each frame without tracking delta time explicitly, update it in `render_instances`:
```rust
        // Decay glitch over time (approximately 60fps)
        if self.glitch_timer > 0.0 {
            self.glitch_timer = (self.glitch_timer - 0.016).max(0.0);
        }
```

Add this block at the top of `render_instances`, before the screen-size updates.

Update `build_postproc_uniform` to use `glitch_timer`:
```rust
            glitch_intensity: self.glitch_timer,
```

- [ ] **Step 4: Build + test**

```bash
~/.cargo/bin/cargo build -p SYNAPSE_-app
```

Add `chroma = { strength = 0.003 }` to config, run app. Expected: RGB channels slightly offset at screen edges.

- [ ] **Step 5: Commit**

```bash
git add crates/SYNAPSE_-renderer/src/shaders/postproc.wgsl \
        crates/SYNAPSE_-renderer/src/renderer.rs
git commit -m "feat(shader): chromatic aberration + glitch/datamosh effect"
```

---

## Task 8: Matrix rain + hex grid backgrounds

**Files:**
- Modify: `crates/SYNAPSE_-renderer/src/shaders/postproc.wgsl`

These effects render BEHIND the terminal content by blending into the cleared background color.

- [ ] **Step 1: Add matrix rain function**

Add to `postproc.wgsl` before `fs_main`:

```wgsl
fn matrix_rain(frag_pos: vec2<f32>, time: f32, density: f32) -> f32 {
    let char_h   = 16.0;  // pixels per character row
    let char_w   = 9.0;   // pixels per column
    let col = floor(frag_pos.x / char_w);
    let row = floor(frag_pos.y / char_h);

    // Per-column pseudo-random speed and offset
    let col_speed  = hash(col * 3.71) * 20.0 + 8.0;
    let col_start  = hash(col * 9.13 + 1.0) * 40.0;
    let trail_len  = hash(col * 2.37 + 2.0) * 12.0 + 4.0;

    // Head row position: wraps every `period` rows
    let period      = 60.0;
    let head_row    = fract((time * col_speed / period + col_start)) * period;
    let dist_from_head = head_row - row;

    // Density gate: only some columns are active
    if hash(col * 1.337 + floor(time * 0.3)) > density { return 0.0; }

    if dist_from_head < 0.0 || dist_from_head > trail_len { return 0.0; }

    // Head cell = full brightness; trail decays
    let t = 1.0 - dist_from_head / trail_len;
    return pow(t, 2.2);
}
```

- [ ] **Step 2: Add hex grid function**

```wgsl
fn hex_dist(p: vec2<f32>) -> f32 {
    let pa = abs(p);
    return max(pa.x * 0.866025 + pa.y * 0.5, pa.y);  // max(c30·x + s30·y, y)
}

fn hex_grid(uv: vec2<f32>, time: f32) -> f32 {
    let scale    = vec2(0.035, 0.06);
    let p        = uv / scale;
    let grid_id  = round(p);
    let local    = p - grid_id;
    let d        = hex_dist(local);
    let edge     = smoothstep(0.42, 0.46, d);
    let pulse    = sin(time * 1.1 + grid_id.x * 0.4 + grid_id.y * 0.7) * 0.5 + 0.5;
    return edge * pulse * 0.12;
}
```

- [ ] **Step 3: Wire both effects in fs_main**

After the bloom block and before the final return in `fs_main`, add:

```wgsl
    // Matrix rain and hex grid use ADDITIVE blending over the scene.
    // Additive is correct: rain appears as phosphorescent light, making
    // dark areas glow green while bright text stays crisp (just slightly tinted).
    if (u.effects_mask & EFFECT_MATRIX_BG) != 0u {
        let intensity = matrix_rain(frag_pos.xy, u.time, u.matrix_density);
        color += u.matrix_color.rgb * intensity * 0.45;
    }

    if (u.effects_mask & EFFECT_HEX_GRID) != 0u {
        let hex_uv = frag_pos.xy / u.screen_size;
        let intensity = hex_grid(hex_uv, u.time);
        color += vec3(0.08, 0.35, 0.9) * intensity;  // electric blue hex edges
    }
```

No alpha compositing needed — additive blend is simpler and visually correct. The clear color (dark bg) means dark regions show rain clearly; bright glyph regions just get a faint tint.

- [ ] **Step 4: Build + test**

```bash
~/.cargo/bin/cargo build -p SYNAPSE_-renderer
```

Add to config:
```toml
[effects.matrix_bg]
enabled = true
color = "#00FF55"
density = 0.35
```

Run app. Expected: green matrix rain visible in empty terminal areas, terminal text on top.

Also test hex grid: set `hex_grid = true` in config.

- [ ] **Step 5: Commit**

```bash
git add crates/SYNAPSE_-renderer/src/shaders/postproc.wgsl
git commit -m "feat(shader): matrix rain + hex grid animated backgrounds"
```

---

## Task 9: Neon pane border pulse animation

The active pane border color is animated using a sin wave. No shader changes needed — this is done by modulating the border rect color in the app render layer.

**Files:**
- Modify: `crates/SYNAPSE_-app/src/render.rs`

- [ ] **Step 1: Find where pane borders are rendered in render.rs**

Search for where `panel_active_border` or similar color is used to build `UIRect` instances for pane borders. It will be in a function like `build_ui_rects` or `render_pane_borders`.

Run:
```bash
grep -n "panel_active_border\|active_border\|pane_border" crates/SYNAPSE_-app/src/render.rs | head -20
```

- [ ] **Step 2: Add time-based pulse to active border color**

Wherever the active pane border `UIRect` is created, replace the static color with a pulsed one.

The pattern is `color: theme.panel_active_border`. Replace with:

```rust
let border_color = if state.effects_enabled && state.config.effects.pane_pulse {
    let t = frame_start.elapsed().as_secs_f32();
    let pulse = (t * std::f32::consts::PI).sin() * 0.5 + 0.5; // 0→1→0 every 2s
    let alpha = 0.6 + pulse * 0.4; // oscillates between 0.6 and 1.0
    let c = theme.panel_active_border;
    [c[0], c[1], c[2], alpha]
} else {
    theme.panel_active_border
};
```

Pass `frame_start: &std::time::Instant` into the render function if not already available. `AppCore` has `last_blink: Instant` — add `render_start: Instant` to `AppCore` or compute time from `start_time`.

Simplest approach: use `std::time::SystemTime` or pass elapsed secs from `AppCore`. Add `pub start_time: Instant` to `AppCore` in `app.rs`, initialized to `Instant::now()` at app creation. Pass `self.start_time.elapsed().as_secs_f32()` into the render function as `time_secs: f32`.

In render.rs, the function signature becomes:
```rust
pub fn build_frame(..., time_secs: f32, ...)
```

Then: `let pulse = (time_secs * std::f32::consts::PI).sin() * 0.5 + 0.5;`

- [ ] **Step 3: Build + verify**

```bash
~/.cargo/bin/cargo build -p SYNAPSE_-app
```

Enable `pane_pulse = true` in config. Run app. Expected: active pane border gently oscillates between 60% and 100% opacity on a ~2-second cycle.

- [ ] **Step 4: Commit**

```bash
git add crates/SYNAPSE_-app/src/render.rs crates/SYNAPSE_-app/src/app.rs
git commit -m "feat(app): neon pane border pulse animation"
```

---

## Task 10: Cursor trail

The cursor trail renders the last N cursor positions as progressively transparent rectangles behind the actual cursor.

**Files:**
- Modify: `crates/SYNAPSE_-app/src/state.rs`
- Modify: `crates/SYNAPSE_-app/src/render.rs`

- [ ] **Step 1: Add cursor trail deque to AppState**

In `state.rs`, add import:
```rust
use std::collections::VecDeque;
```

Add to `AppState` struct:
```rust
    pub cursor_trail: VecDeque<(f32, f32)>,  // pixel (x, y) of last N cursor positions
```

Initialize in `AppState::new`:
```rust
            cursor_trail: VecDeque::new(),
```

Add a method:
```rust
    pub fn push_cursor_trail(&mut self, x: f32, y: f32, max_len: usize) {
        if max_len == 0 { return; }
        if self.cursor_trail.front() != Some(&(x, y)) {
            self.cursor_trail.push_front((x, y));
            while self.cursor_trail.len() > max_len {
                self.cursor_trail.pop_back();
            }
        }
    }
```

- [ ] **Step 2: Push cursor position each frame**

In `render.rs` (or wherever the cursor `UIRect` is built), after the cursor pixel position is computed, call:
```rust
    state.push_cursor_trail(cursor_px, cursor_py, state.config.effects.cursor_trail as usize);
```

Where `cursor_px` and `cursor_py` are the pixel-space cursor top-left position.

- [ ] **Step 3: Render trail rects**

In `render.rs`, where cursor rects are collected into `ui_rects`, add before the actual cursor rect:

```rust
    if state.config.effects.cursor_trail > 0 && state.effects_enabled {
        let trail_len = state.cursor_trail.len();
        for (i, &(tx, ty)) in state.cursor_trail.iter().enumerate().skip(1) {
            let alpha = 1.0 - (i as f32 / trail_len as f32);
            let faded_color = [
                theme.cursor[0],
                theme.cursor[1],
                theme.cursor[2],
                theme.cursor[3] * alpha * 0.6,
            ];
            ui_rects.push(UIRect {
                pos:   [tx, ty],
                size:  [cell_w, cell_h],
                color: faded_color,
            });
        }
    }
```

Make sure trail rects are pushed BEFORE the actual cursor rect so they render behind it.

- [ ] **Step 4: Write tests**

In `state.rs` `#[cfg(test)]` block:
```rust
    #[test]
    fn test_cursor_trail_push() {
        let mut state = AppState::new(/* ... */);
        state.push_cursor_trail(10.0, 20.0, 4);
        state.push_cursor_trail(15.0, 20.0, 4);
        state.push_cursor_trail(20.0, 20.0, 4);
        assert_eq!(state.cursor_trail.len(), 3);
        assert_eq!(state.cursor_trail[0], (20.0, 20.0));

        // Stays within max_len
        state.push_cursor_trail(25.0, 20.0, 4);
        state.push_cursor_trail(30.0, 20.0, 4);
        assert_eq!(state.cursor_trail.len(), 4);
    }

    #[test]
    fn test_cursor_trail_no_duplicate() {
        let mut state = AppState::new(/* ... */);
        state.push_cursor_trail(10.0, 10.0, 4);
        state.push_cursor_trail(10.0, 10.0, 4);  // same position
        assert_eq!(state.cursor_trail.len(), 1);
    }
```

Note: `AppState::new` requires Config and other params — adapt the test to match AppState's actual constructor signature.

- [ ] **Step 5: Run tests**

```bash
~/.cargo/bin/cargo test -p SYNAPSE_-app cursor_trail -- --nocapture
```

Expected: tests pass.

- [ ] **Step 6: Build + verify**

```bash
~/.cargo/bin/cargo build -p SYNAPSE_-app
```

Set `cursor_trail = 4` in config, run app. Move cursor rapidly. Expected: 4 fading ghost cursor rectangles trailing behind cursor movement.

- [ ] **Step 7: Commit**

```bash
git add crates/SYNAPSE_-app/src/state.rs crates/SYNAPSE_-app/src/render.rs
git commit -m "feat(app): cursor trail — N fading ghost cursors behind active cursor"
```

---

## Final integration test

After all 10 tasks are complete:

- [ ] Run full test suite:
```bash
~/.cargo/bin/cargo test --workspace -- --nocapture
```
Expected: all tests pass.

- [ ] Run clippy:
```bash
~/.cargo/bin/cargo clippy --workspace --all-targets -- -D warnings
```
Expected: no warnings.

- [ ] Build release:
```bash
~/.cargo/bin/cargo build --release -p SYNAPSE_-app
```

- [ ] Full config test — add to `~/.config/SYNAPSE_/config.toml`:
```toml
[effects]
enabled = true
pane_pulse = true
cursor_trail = 4
hex_grid = false

[effects.scanlines]
intensity = 0.3
freq = 2.0

[effects.bloom]
threshold = 0.65
sigma = 3.5
tint = "#FF003C"

[effects.chroma]
strength = 0.002

[effects.matrix_bg]
enabled = false
color = "#00FF55"
density = 0.3
```

Run app. Test:
1. Effects ON by default (enabled = true in config).
2. Toggle with Ctrl+Shift+E → effects disappear (passthrough only).
3. Toggle again → effects return.
4. Edit config, press Ctrl+, → effects hot-reload without restart.
5. Open nvim, run btop, run lazygit — confirm no visual regression.

---

## Spec compliance checklist (13.1–13.12)

| Spec item | Task | Status |
|-----------|------|--------|
| 13.1 Postproc pipeline (offscreen tex + 2nd pass) | Task 3+4 | ✅ |
| 13.2 CRT scanlines (intensity, freq, barrel, vignette) | Task 5 | ✅ |
| 13.3 Bloom/glow (2-pass gaussian, threshold, tint, sigma) | Task 6 | ✅ |
| 13.4 Chromatic aberration (radial RGB split) | Task 7 | ✅ |
| 13.5 Glitch/datamosh (horizontal shifts, time-based) | Task 7 | ✅ |
| 13.6 Phosphor decay | Not in this plan — deferred to Phase 13b |
| 13.7 Matrix rain background | Task 8 | ✅ |
| 13.8 Hex grid background | Task 8 | ✅ |
| 13.9 Neon pane border pulse | Task 9 | ✅ |
| 13.10 Cursor trail | Task 10 | ✅ |
| 13.11 Config TOML `[effects]` block | Task 1 | ✅ |
| 13.12 Keybind `effects_toggle` (Ctrl+Shift+E) | Task 2 | ✅ |

**Deferred:** 13.6 Phosphor decay (text fade-in on rapid output) requires per-cell timestamp tracking in the render buffer — too invasive for Phase 13. Schedule as Phase 13b.
