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
