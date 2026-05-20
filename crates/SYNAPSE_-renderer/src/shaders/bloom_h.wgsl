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
    let uv = frag_pos.xy / u.screen_size;
    let pixel_w = 1.0 / u.screen_size.x;

    var result = vec3(0.0);
    var weight_sum = 0.0;

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
