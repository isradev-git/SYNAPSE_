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
