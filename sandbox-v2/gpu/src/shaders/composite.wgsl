// Final composite into the target: scene + BLOOM_STRENGTH * bloom (both
// sampled bilinearly, stretched to the target), times EXPOSURE, ACES filmic
// fit (Narkowicz 2015), clamped to 0..1, non-finite values become 0. When the
// target format is not sRGB the shader applies the sRGB transfer itself
// (SRGB_ENCODE), so every target shows the same image.
//
// Bindings: 0 scene texture, 1 bloom texture, 2 linear sampler,
// 3 target-size uniform.

@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var bloom: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var<uniform> target_size: vec4<f32>;

fn aces(x: vec3<f32>) -> vec3<f32> {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        return 12.92 * c;
    }
    return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}

@fragment
fn composite(@builtin(position) fc: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = fc.xy / target_size.xy;
    let hdr = textureSampleLevel(scene, samp, uv, 0.0).rgb
        + BLOOM_STRENGTH * textureSampleLevel(bloom, samp, uv, 0.0).rgb;
    var c = aces(max(finite3(hdr), vec3<f32>(0.0)) * EXPOSURE);
    if SRGB_ENCODE {
        c = vec3<f32>(srgb(c.x), srgb(c.y), srgb(c.z));
    }
    return vec4<f32>(clamp(finite3(c), vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
