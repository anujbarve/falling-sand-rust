// Light field (cell resolution), three stages:
// 1. `emit`: every cell's blackbody emission and solid flag into `emis`
//    (rgb = emission, a = 1 if solid); every emitting cell seeds itself as
//    its own light source in the source map. The map holds TWO sources per
//    cell: rg = the brightest (largest luma(E) * falloff(d)), ba = the
//    nearest emitter; -1 = none.
// 2. `jump` (jump flooding, Rong & Tan 2006), one pass per step in
//    LIGHT_STEPS: each cell looks at both sources found by itself and its 8
//    neighbours `step` cells away and keeps the brightest and the nearest.
// 3. `resolve`: for each of the two sources, E(source) * falloff(d), zero if
//    a solid cell lies on the straight line between the cell and the source
//    (ray-marched occluder test, at most LIGHT_RADIUS steps): hard shadows.
//    Light = max of the two (and the cell's own emission). Keeping the
//    nearest as well as the brightest means a cell whose brightest emitter
//    is hidden is still lit by a visible nearer one.
// falloff(d) = clamp(1 - (d/R)^4, 0, 1)^2 / (1 + (d/D0)^2), R = LIGHT_RADIUS,
// D0 = LIGHT_D0: inverse-square with a smooth window to exactly 0 at R
// (Karis 2013, "Real Shading in Unreal Engine 4"), so no hard reach edge.
//
// Bindings: 0 view uniform, 1 cells (storage, read; emit and resolve),
// 2 destination (storage texture, write), 3 emis texture, 4 source texture
// (jump and resolve), 5 seed destination (storage texture; emit), 6 step
// uniform (jump).

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<storage, read> cells: array<Cell>;
@group(0) @binding(2) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var emis: texture_2d<f32>;
@group(0) @binding(4) var src: texture_2d<f32>;
@group(0) @binding(5) var seed: texture_storage_2d<rgba16float, write>;
@group(0) @binding(6) var<uniform> step: vec4<i32>;

fn falloff(d: f32) -> f32 {
    let r = d / LIGHT_RADIUS;
    let win = clamp(1.0 - r * r * r * r, 0.0, 1.0);
    let q = d / LIGHT_D0;
    return win * win / (1.0 + q * q);
}

fn inside(q: vec2<i32>) -> bool {
    return q.x >= 0 && q.y >= 0 && q.x < i32(view.w) && q.y < i32(view.h);
}

@compute @workgroup_size(8, 8)
fn emit(@builtin(global_invocation_id) g: vec3<u32>) {
    if g.x >= view.w || g.y >= view.h {
        return;
    }
    let c = cells[g.y * view.w + g.x];
    let e = blackbody(safe_temp(c.temp));
    let solid = select(0.0, 1.0, is_solid(min(c.mat, WALL)));
    let p = vec2<i32>(g.xy);
    textureStore(dst, p, vec4<f32>(e, solid));
    var s = vec4<f32>(-1.0);
    if luma(e) > 0.0 {
        s = vec4<f32>(vec2<f32>(p), vec2<f32>(p));
    }
    textureStore(seed, p, s);
}

struct Pick {
    bright: vec2<f32>,
    bright_score: f32,
    near: vec2<f32>,
    near_d: f32,
}

fn consider(k: ptr<function, Pick>, s: vec2<f32>, p: vec2<i32>) {
    if s.x < 0.0 {
        return;
    }
    let d = length(s - vec2<f32>(p));
    if d >= LIGHT_RADIUS {
        return;
    }
    let score = luma(textureLoad(emis, vec2<i32>(s), 0).rgb) * falloff(d);
    if score > (*k).bright_score {
        (*k).bright_score = score;
        (*k).bright = s;
    }
    if d < (*k).near_d {
        (*k).near_d = d;
        (*k).near = s;
    }
}

@compute @workgroup_size(8, 8)
fn jump(@builtin(global_invocation_id) g: vec3<u32>) {
    if g.x >= view.w || g.y >= view.h {
        return;
    }
    let p = vec2<i32>(g.xy);
    var k = Pick(vec2<f32>(-1.0), 0.0, vec2<f32>(-1.0), LIGHT_RADIUS);
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let q = p + vec2<i32>(i, j) * step.x;
            if !inside(q) {
                continue;
            }
            let s = textureLoad(src, q, 0);
            consider(&k, s.xy, p);
            consider(&k, s.zw, p);
        }
    }
    textureStore(dst, p, vec4<f32>(k.bright, k.near));
}

// Light from source `s` at cell `p`, or 0 if `s` is none or hidden.
fn shine(p: vec2<i32>, s: vec2<f32>) -> vec3<f32> {
    if s.x < 0.0 {
        return vec3<f32>(0.0);
    }
    let sp = vec2<i32>(s);
    let dv = vec2<f32>(sp - p);
    let n = i32(ceil(max(abs(dv.x), abs(dv.y))));
    for (var i = 1; i < n; i++) {
        let q = vec2<i32>(floor(vec2<f32>(p) + 0.5 + dv * (f32(i) / f32(n))));
        if all(q == sp) {
            break;
        }
        if is_solid(min(cells[u32(q.y) * view.w + u32(q.x)].mat, WALL)) {
            return vec3<f32>(0.0);
        }
    }
    return textureLoad(emis, sp, 0).rgb * falloff(length(dv));
}

@compute @workgroup_size(8, 8)
fn resolve(@builtin(global_invocation_id) g: vec3<u32>) {
    if g.x >= view.w || g.y >= view.h {
        return;
    }
    let p = vec2<i32>(g.xy);
    let own = textureLoad(emis, p, 0);
    let s = textureLoad(src, p, 0);
    var l = max(own.rgb, shine(p, s.xy));
    if any(s.zw != s.xy) {
        l = max(l, shine(p, s.zw));
    }
    textureStore(dst, p, vec4<f32>(l, own.a));
}
