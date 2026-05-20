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
    screen_size:      vec2<f32>,
    time:             f32,
    effects_mask:     u32,
    scanline_intensity: f32,
    scanline_freq:    f32,
    bloom_threshold:  f32,
    bloom_sigma:      f32,
    bloom_tint:       vec4<f32>,
    chroma_strength:  f32,
    glitch_intensity: f32,
    matrix_density:   f32,
    _pad:             f32,
    matrix_color:     vec4<f32>,
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

fn hash(n: f32) -> f32 {
    return fract(sin(n) * 43758.5453123);
}

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
    let line = sin(screen_y * 3.14159265 * freq) * 0.5 + 0.5;
    return color * (1.0 - intensity * (1.0 - line));
}

@fragment
fn fs_main(@builtin(position) frag_pos: vec4<f32>) -> @location(0) vec4<f32> {
    var uv = frag_pos.xy / u.screen_size;

    if (u.effects_mask & EFFECT_SCANLINES) != 0u {
        uv = barrel_uv(uv, 0.05);
        uv = clamp(uv, vec2(0.001), vec2(0.999));
    }

    var color = textureSample(scene_tex, scene_sampler, uv).rgb;

    if (u.effects_mask & EFFECT_SCANLINES) != 0u {
        color = apply_scanlines(color, frag_pos.y, u.scanline_intensity, u.scanline_freq);
        color = color * apply_vignette(uv);
    }

    return vec4(color, 1.0);
}
