struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) fg_color: vec4<f32>,
    @location(2) @interpolate(flat) bg_color: vec4<f32>,
}

struct CellInstance {
    cell_pos: vec2<f32>,
    cell_size: vec2<f32>,
    uv_rect: vec4<f32>,
    fg_color: vec4<f32>,
    bg_color: vec4<f32>,
}

struct ScreenUniform {
    screen_size: vec2<f32>,
}

@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;
@group(1) @binding(0) var<uniform> screen: ScreenUniform;

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) instance: CellInstance,
) -> VertexOutput {
    let corners = array<vec2<f32>, 4>(
        vec2(0.0, 0.0),
        vec2(1.0, 0.0),
        vec2(1.0, 1.0),
        vec2(0.0, 1.0),
    );
    let corner = corners[vertex_index];

    let pixel_pos = instance.cell_pos + corner * instance.cell_size;

    var out: VertexOutput;
    out.position = vec4(
        (pixel_pos.x / screen.screen_size.x) * 2.0 - 1.0,
        1.0 - (pixel_pos.y / screen.screen_size.y) * 2.0,
        0.0,
        1.0,
    );
    let uv_top_left = instance.uv_rect.xy;
    let uv_size = instance.uv_rect.zw - instance.uv_rect.xy;
    out.uv = uv_top_left + corner * uv_size;
    out.fg_color = instance.fg_color;
    out.bg_color = instance.bg_color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let atlas_color = textureSample(atlas, atlas_sampler, in.uv);
    let alpha = atlas_color.a;
    return mix(in.bg_color, in.fg_color, alpha);
}
