// Lava pass: viscosity countdown (in place on dst), then two row-parity move
// phases (src -> dst). Mirrors reference_thermal::ThermalReference::lava.
fn mat_at(x: i32, y: i32) -> u32 {
    if (!inside(x, y)) { return WALL; }
    return src[lin(x, y)].mat;
}

fn is_empty(x: i32, y: i32) -> bool { return mat_at(x, y) == EMPTY; }

@compute @workgroup_size(8, 8)
fn stage_countdown(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.w || gid.y >= params.h) { return; }
    let i = gid.y * params.w + gid.x;
    if (dst[i].mat != LAVA) { return; }
    var f = 0.0;
    if (finite(dst[i].fuel)) { f = floor(dst[i].fuel); }
    dst[i].fuel = min(max(f - 1.0, 0.0), LAVA_DELAY - 1.0);
}

// Linear index of the target of the lava at (x, y) this phase, or -1.
fn plan(x: i32, y: i32) -> i32 {
    if (!inside(x, y) || (u32(y) & 1u) != params.phase || mat_at(x, y) != LAVA) { return -1; }
    let f = src[lin(x, y)].fuel;
    if (!(f < LAVA_DELAY)) { return -1; }
    if (is_empty(x, y + 1)) { return i32(lin(x, y + 1)); }
    if (!(f < 1.0)) { return -1; }
    let h = cell_hash(params.tick, u32(x), u32(y), params.seed);
    var s = -1;
    if ((h & 1u) == 1u) { s = 1; }
    if (is_empty(x + s, y + 1)) { return i32(lin(x + s, y + 1)); }
    if (is_empty(x - s, y + 1)) { return i32(lin(x - s, y + 1)); }
    if (mat_at(x, y - 1) == LAVA) {
        var q = -1;
        if (((h >> 1u) & 1u) == 1u) { q = 1; }
        if (is_empty(x + q, y)) { return i32(lin(x + q, y)); }
        if (is_empty(x - q, y)) { return i32(lin(x - q, y)); }
    }
    return -1;
}

// Linear index of the lava that wins the empty cell (x, y), or -1:
// above, above-left, above-right, left, right.
fn winner(x: i32, y: i32) -> i32 {
    let me = i32(lin(x, y));
    let cand = array<vec2<i32>, 5>(vec2(0, -1), vec2(-1, -1), vec2(1, -1), vec2(-1, 0), vec2(1, 0));
    for (var k = 0; k < 5; k += 1) {
        let sx = x + cand[k].x;
        let sy = y + cand[k].y;
        if (plan(sx, sy) == me) { return i32(lin(sx, sy)); }
    }
    return -1;
}

@compute @workgroup_size(8, 8)
fn stage_move(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.w || gid.y >= params.h) { return; }
    let x = i32(gid.x);
    let y = i32(gid.y);
    let i = lin(x, y);
    var out = src[i];
    if (out.mat == EMPTY) {
        let s = winner(x, y);
        if (s >= 0) {
            out = src[u32(s)];
            out.fuel = LAVA_DELAY;
            atomicAdd(&events[0], 1u);
        }
    } else {
        let t = plan(x, y);
        if (t >= 0) {
            let tx = t % i32(params.w);
            let ty = t / i32(params.w);
            if (winner(tx, ty) == i32(i)) { out = src[u32(t)]; }
        }
    }
    dst[i] = out;
}
