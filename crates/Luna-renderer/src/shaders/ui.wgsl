struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
}

struct UIRect {
    pos: vec2<f32>,
    size: vec2<f32>,
    color: vec4<f32>,
}

struct ScreenUniform {
    screen_size: vec2<f32>,
}

@group(0) @binding(0) var<uniform> screen: ScreenUniform;

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) rect: UIRect,
) -> VertexOutput {
    let corners = array<vec2<f32>, 4>(
        vec2(0.0, 0.0),
        vec2(1.0, 0.0),
        vec2(1.0, 1.0),
        vec2(0.0, 1.0),
    );
    let corner = corners[vertex_index];
    let pixel_pos = rect.pos + corner * rect.size;

    var out: VertexOutput;
    out.position = vec4(
        (pixel_pos.x / screen.screen_size.x) * 2.0 - 1.0,
        1.0 - (pixel_pos.y / screen.screen_size.y) * 2.0,
        0.0,
        1.0,
    );
    out.color = rect.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
