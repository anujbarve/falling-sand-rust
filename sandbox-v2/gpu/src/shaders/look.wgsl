// Shared look helpers for the render shaders. Concatenated after the header
// generated in render.rs (constants, ALBEDO, BB_T/BB_C, is_solid, is_liquid),
// common.wgsl and hash.wgsl.

struct View {
    w: u32,
    h: u32,
    scale: u32,
    seed: u32,
}

// Bit-pattern finiteness test (not x != x, which fast-math may fold).
fn finite(v: f32) -> bool { return (bitcast<u32>(v) & 0x7f800000u) != 0x7f800000u; }

fn finite3(v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(select(0.0, v.x, finite(v.x)), select(0.0, v.y, finite(v.y)), select(0.0, v.z, finite(v.z)));
}

fn safe_temp(t: f32) -> f32 {
    if finite(t) { return clamp(t, 0.0, 10000.0); }
    return AMBIENT_K;
}

// Deterministic per-cell noise in [0, 1) from (x, y, NOISE_SEED).
fn hash01(x: u32, y: u32) -> f32 {
    return f32(cell_hash(0u, x, y, NOISE_SEED) >> 8u) * (1.0 / 16777216.0);
}

fn albedo(m: u32) -> vec4<f32> {
    var t = ALBEDO;
    return t[min(m, WALL)];
}

// Blackbody emission: colour piecewise-linear in the BB table (normalised
// linear sRGB), intensity EMIT_GAIN * (T / EMIT_REF_K)^4 (Stefan-Boltzmann)
// faded in over EMIT_MIN_K..EMIT_FULL_K, capped at EMIT_MAX.
fn blackbody(t: f32) -> vec3<f32> {
    if t <= EMIT_MIN_K { return vec3<f32>(0.0); }
    var tt = BB_T;
    var cc = BB_C;
    var c = cc[BB_N - 1];
    for (var i = 1; i < BB_N; i++) {
        if t < tt[i] {
            c = mix(cc[i - 1], cc[i], (t - tt[i - 1]) / (tt[i] - tt[i - 1]));
            break;
        }
    }
    let s = t / EMIT_REF_K;
    let k = EMIT_GAIN * s * s * s * s * smoothstep(EMIT_MIN_K, EMIT_FULL_K, t);
    return c * min(k, EMIT_MAX);
}

fn luma(c: vec3<f32>) -> f32 { return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722)); }
