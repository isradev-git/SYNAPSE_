struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct ScreenUniform {
    screen_size: vec2<f32>,
}

@group(0) @binding(0) var<uniform> screen: ScreenUniform;
@group(1) @binding(0) var image_tex: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;

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
    out.uv = corner;
    return out;
}

@fragment
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(image_tex, image_sampler, uv);
}
