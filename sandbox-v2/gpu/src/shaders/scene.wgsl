// Scene pass (HDR, view.scale pixels per cell). `fill_fs` upsamples the
// shaded cells with a sharpened bilinear filter: inside a cell the colour is
// flat, across a cell boundary it blends over about EDGE_PX pixels, so cells
// read as smooth shapes instead of hard blocks. `sprite_vs/fs` draw each
// live particle as an instanced round sprite (radius PART_RADIUS; when
// hotter than EMIT_MIN_K a soft glow of radius PART_GLOW_RADIUS) in its material colour, lit
// by the light field, plus blackbody emission.
//
// Bindings: 0 view uniform, 1 shaded cells texture, 2 linear sampler,
// 3 light texture (sprite vertex stage). Particles arrive as a per-instance
// vertex buffer (no storage binding).

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var cellcol: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var light: texture_2d<f32>;

@fragment
fn fill_fs(@builtin(position) fc: vec4<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(f32(view.w), f32(view.h));
    let q = fc.xy / f32(view.scale) - 0.5;
    let i = floor(q);
    let sharp = max(f32(view.scale) / EDGE_PX, 1.0);
    let f = clamp((q - i - 0.5) * sharp + 0.5, vec2<f32>(0.0), vec2<f32>(1.0));
    let uv = (i + 0.5 + f) / dims;
    return vec4<f32>(textureSampleLevel(cellcol, samp, uv, 0.0).rgb, 1.0);
}

struct SpriteOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) corner: vec2<f32>,
    @location(1) col: vec3<f32>,
    @location(2) glow: f32,
}

@vertex
fn sprite_vs(
    @builtin(vertex_index) vi: u32,
    @location(0) pos: vec2<f32>,
    @location(1) mat: u32,
    @location(2) temp: f32,
    @location(3) life: f32,
) -> SpriteOut {
    var out: SpriteOut;
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u)) * 2.0 - 1.0;
    out.corner = corner;
    out.col = vec3<f32>(0.0);
    out.glow = 0.0;
    if !(life > 0.0) || !finite(pos.x) || !finite(pos.y) {
        out.pos = vec4<f32>(-4.0, -4.0, 0.0, 1.0);
        return out;
    }
    let t = safe_temp(temp);
    var r = PART_RADIUS;
    if t > EMIT_MIN_K {
        r = PART_GLOW_RADIUS;
        out.glow = 1.0;
    }
    let cellp = clamp(vec2<i32>(floor(pos)), vec2<i32>(0), vec2<i32>(i32(view.w) - 1, i32(view.h) - 1));
    let l = textureLoad(light, cellp, 0).rgb;
    out.col = finite3(albedo(min(mat, WALL)).rgb * (AMBIENT + LIGHT_GAIN * l) + blackbody(t));
    let world = pos + corner * r;
    out.pos = vec4<f32>(world.x / f32(view.w) * 2.0 - 1.0, 1.0 - world.y / f32(view.h) * 2.0, 0.0, 1.0);
    return out;
}

@fragment
fn sprite_fs(in: SpriteOut) -> @location(0) vec4<f32> {
    let d = length(in.corner);
    // Cold: a solid round grain. Hot: a soft glow, alpha (1 - d)^2.
    let halo = clamp(1.0 - d, 0.0, 1.0);
    let a = mix(1.0 - smoothstep(0.5, 1.0, d), halo * halo, in.glow);
    return vec4<f32>(in.col, a);
}
