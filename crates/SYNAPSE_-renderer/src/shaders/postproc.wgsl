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

fn apply_chroma(uv: vec2<f32>, strength: f32) -> vec3<f32> {
    let center = uv - 0.5;
    let dist = length(center);
    let dir = normalize(center + vec2(0.0001));
    let r = textureSample(scene_tex, scene_sampler, clamp(uv + dir * strength * dist, vec2(0.0), vec2(1.0))).r;
    let g = textureSample(scene_tex, scene_sampler, uv).g;
    let b = textureSample(scene_tex, scene_sampler, clamp(uv - dir * strength * dist, vec2(0.0), vec2(1.0))).b;
    return vec3(r, g, b);
}

fn apply_glitch(color: vec3<f32>, uv: vec2<f32>, time: f32, intensity: f32) -> vec3<f32> {
    if intensity <= 0.001 { return color; }
    let block_y  = floor(uv.y * 24.0);
    let glitch_t = floor(time * 12.0);
    let noise    = hash(block_y * 13.7 + glitch_t * 0.31);
    if noise > 1.0 - intensity * 0.4 {
        let shift    = (hash(block_y * 7.3 + glitch_t) - 0.5) * intensity * 0.08;
        let glitch_uv = vec2(fract(uv.x + shift), uv.y);
        return textureSample(scene_tex, scene_sampler, glitch_uv).rgb;
    }
    return color;
}

fn matrix_rain(frag_pos: vec2<f32>, time: f32, density: f32) -> f32 {
    let char_h   = 16.0;
    let char_w   = 9.0;
    let col = floor(frag_pos.x / char_w);
    let row = floor(frag_pos.y / char_h);

    let col_speed  = hash(col * 3.71) * 20.0 + 8.0;
    let col_start  = hash(col * 9.13 + 1.0) * 40.0;
    let trail_len  = hash(col * 2.37 + 2.0) * 12.0 + 4.0;

    let period      = 60.0;
    let head_row    = fract((time * col_speed / period + col_start)) * period;
    let dist_from_head = head_row - row;

    if hash(col * 1.337 + floor(time * 0.3)) > density { return 0.0; }
    if dist_from_head < 0.0 || dist_from_head > trail_len { return 0.0; }

    let t = 1.0 - dist_from_head / trail_len;
    return pow(t, 2.2);
}

fn hex_dist(p: vec2<f32>) -> f32 {
    let pa = abs(p);
    return max(pa.x * 0.866025 + pa.y * 0.5, pa.y);
}

fn hex_grid(uv: vec2<f32>, time: f32) -> f32 {
    let scale   = vec2(0.035, 0.06);
    let p       = uv / scale;
    let grid_id = round(p);
    let local   = p - grid_id;
    let d       = hex_dist(local);
    let edge    = smoothstep(0.42, 0.46, d);
    let pulse   = sin(time * 1.1 + grid_id.x * 0.4 + grid_id.y * 0.7) * 0.5 + 0.5;
    return edge * pulse * 0.12;
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

    if (u.effects_mask & EFFECT_BLOOM) != 0u {
        let bloom_v = sample_bloom_v(uv, u.bloom_sigma);
        let tinted = bloom_v * u.bloom_tint.rgb * 2.0;
        color = color + tinted;
        color = color / (color + vec3(1.0));
    }

    if (u.effects_mask & EFFECT_CHROMA) != 0u {
        color = apply_chroma(uv, u.chroma_strength);
    }

    if u.glitch_intensity > 0.001 {
        color = apply_glitch(color, uv, u.time, u.glitch_intensity);
    }

    if (u.effects_mask & EFFECT_MATRIX_BG) != 0u {
        let intensity = matrix_rain(frag_pos.xy, u.time, u.matrix_density);
        color += u.matrix_color.rgb * intensity * 0.45;
    }

    if (u.effects_mask & EFFECT_HEX_GRID) != 0u {
        let hex_uv = frag_pos.xy / u.screen_size;
        let intensity = hex_grid(hex_uv, u.time);
        color += vec3(0.08, 0.35, 0.9) * intensity;
    }

    return vec4(color, 1.0);
}
