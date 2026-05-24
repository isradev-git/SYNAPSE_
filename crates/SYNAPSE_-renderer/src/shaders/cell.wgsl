struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) fg_color: vec4<f32>,
    @location(2) @interpolate(flat) bg_color: vec4<f32>,
    @location(3) @interpolate(flat) is_emoji: u32,
}

struct ScreenUniform {
    screen_size: vec2<f32>,
}

@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;
@group(1) @binding(0) var<uniform> screen: ScreenUniform;

fn corner_for_index(idx: u32) -> vec2<f32> {
    if (idx == 0u) { return vec2(0.0, 0.0); } // top-left
    if (idx == 1u) { return vec2(1.0, 0.0); } // top-right
    if (idx == 2u) { return vec2(0.0, 1.0); } // bottom-left
    return vec2(1.0, 1.0);                    // bottom-right
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) cell_pos: vec2<f32>,
    @location(1) cell_size: vec2<f32>,
    @location(2) uv_rect: vec4<f32>,
    @location(3) fg_color: vec4<f32>,
    @location(4) bg_color: vec4<f32>,
    @location(5) is_emoji: u32,
) -> VertexOutput {
    let corner = corner_for_index(vertex_index);

    let pixel_pos = cell_pos + corner * cell_size;

    var out: VertexOutput;
    out.position = vec4(
        (pixel_pos.x / screen.screen_size.x) * 2.0 - 1.0,
        1.0 - (pixel_pos.y / screen.screen_size.y) * 2.0,
        0.0,
        1.0,
    );
    let uv_top_left = uv_rect.xy;
    let uv_size = uv_rect.zw - uv_rect.xy;
    out.uv = uv_top_left + corner * uv_size;
    out.fg_color = fg_color;
    out.bg_color = bg_color;
    out.is_emoji = is_emoji;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(atlas, atlas_sampler, in.uv);
    // Color emoji: atlas stores premultiplied RGBA, output directly.
    if (in.is_emoji != 0u) {
        return sampled;
    }
    // Atlas stores regular glyphs as (1, 1, 1, coverage).
    // Composite fg over bg manually, then premultiply.
    let coverage = sampled.a;
    let composed_rgb = mix(in.bg_color.rgb * in.bg_color.a, in.fg_color.rgb, coverage);
    let composed_a = mix(in.bg_color.a, 1.0, coverage);
    return vec4(composed_rgb, composed_a);
}