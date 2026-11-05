// Bloom at quarter scene resolution. `down` averages a 4x4 block of the
// scene (4 bilinear taps) and keeps only the part brighter than
// BLOOM_THRESHOLD (luminance, soft: c * max(Y - T, 0) / Y). `blur_h` and
// `blur_v` are the two halves of a separable Gaussian (sigma BLOOM_SIGMA,
// radius BLOOM_RADIUS texels).
//
// Bindings: 0 source texture, 1 linear clamp sampler.

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn down(@builtin(position) fc: vec4<f32>) -> @location(0) vec4<f32> {
    let px = 1.0 / vec2<f32>(textureDimensions(src));
    let uv = fc.xy * 4.0 * px;
    var c = vec3<f32>(0.0);
    c += textureSampleLevel(src, samp, uv + vec2<f32>(-1.0, -1.0) * px, 0.0).rgb;
    c += textureSampleLevel(src, samp, uv + vec2<f32>(1.0, -1.0) * px, 0.0).rgb;
    c += textureSampleLevel(src, samp, uv + vec2<f32>(-1.0, 1.0) * px, 0.0).rgb;
    c += textureSampleLevel(src, samp, uv + vec2<f32>(1.0, 1.0) * px, 0.0).rgb;
    c = finite3(c * 0.25);
    let y = luma(c);
    return vec4<f32>(c * max(y - BLOOM_THRESHOLD, 0.0) / max(y, 1e-4), 1.0);
}

fn blur(fc: vec4<f32>, dir: vec2<f32>) -> vec4<f32> {
    let px = 1.0 / vec2<f32>(textureDimensions(src));
    let uv = fc.xy * px;
    var sum = vec3<f32>(0.0);
    var norm = 0.0;
    for (var i = -BLOOM_RADIUS; i <= BLOOM_RADIUS; i++) {
        let w = exp(-f32(i * i) / (2.0 * BLOOM_SIGMA * BLOOM_SIGMA));
        sum += w * textureSampleLevel(src, samp, uv + dir * f32(i) * px, 0.0).rgb;
        norm += w;
    }
    return vec4<f32>(sum / norm, 1.0);
}

@fragment
fn blur_h(@builtin(position) fc: vec4<f32>) -> @location(0) vec4<f32> {
    return blur(fc, vec2<f32>(1.0, 0.0));
}

@fragment
fn blur_v(@builtin(position) fc: vec4<f32>) -> @location(0) vec4<f32> {
    return blur(fc, vec2<f32>(0.0, 1.0));
}
