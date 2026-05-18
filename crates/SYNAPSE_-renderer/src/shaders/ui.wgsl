struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
}

struct ScreenUniform {
    screen_size: vec2<f32>,
}

@group(0) @binding(0) var<uniform> screen: ScreenUniform;

// Z-order for TriangleStrip: TL, TR, BL, BR.
// CW order (TL,TR,BR,BL) produces a bowtie that only covers half the rect,
// which is what made tabs look like triangular pennants.
fn corner_for_index(idx: u32) -> vec2<f32> {
    if (idx == 0u) { return vec2(0.0, 0.0); } // top-left
    if (idx == 1u) { return vec2(1.0, 0.0); } // top-right
    if (idx == 2u) { return vec2(0.0, 1.0); } // bottom-left
    return vec2(1.0, 1.0);                    // bottom-right
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
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
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
