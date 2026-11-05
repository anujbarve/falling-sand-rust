// Shared row-parity kernel for granular and liquid passes. Concatenated after
// common.wgsl and after a generated header (see pass_rows.rs) that defines the
// i32 constants FP_ONE, MAX_STEP, GRAVITY_FP, TERMINAL_FP, the u32 constants
// MOVER, EMPTY and WALL (material ids) and the bool SIDEWAYS. Mirrors src/reference.rs exactly (integer math).
struct Params {
    w: u32,
    h: u32,
    tick: u32,
    seed: u32,
    phase: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> src: array<Cell>;
@group(0) @binding(2) var<storage, read_write> dst: array<Cell>;


// murmur3 fmix32, u32 wrapping math (WGSL u32 ops wrap).
fn mix32(v: u32) -> u32 {
    var h = v;
    h ^= h >> 16u;
    h = h * 0x85EBCA6Bu;
    h ^= h >> 13u;
    h = h * 0xC2B2AE35u;
    return h ^ (h >> 16u);
}

// Per-cell hash of (tick, x, y, seed). Bit 0: diagonal side; bit 1: sideways.
fn cell_hash(tick: u32, x: u32, y: u32, seed: u32) -> u32 {
    var h = mix32(seed ^ tick);
    h = mix32(h ^ x);
    return mix32(h ^ y);
}

// Material at (x, y); outside the grid reads as WALL.
fn mat_at(x: i32, y: i32) -> u32 {
    if (x < 0 || y < 0 || x >= i32(params.w) || y >= i32(params.h)) {
        return WALL;
    }
    return src[u32(y) * params.w + u32(x)].mat;
}

fn is_empty(x: i32, y: i32) -> bool {
    return mat_at(x, y) == EMPTY;
}

// Linear index of the cell (x, y); caller guarantees it is in the grid.
fn lin(x: i32, y: i32) -> i32 {
    return y * i32(params.w) + x;
}

// What a mover considered this phase wants. `dest` is a linear index or -1.
// (mv, mf) is the (vy, frac) after a successful move, (sv, sf) when it stays.
struct Plan {
    live: bool,
    dest: i32,
    mv: i32,
    mf: i32,
    sv: i32,
    sf: i32,
}

// Diagonal-down (then, if SIDEWAYS, sideways) dest of a blocked mover.
// Shared by sand and water. Returns a linear index or -1.
fn slide(x: i32, y: i32) -> i32 {
    let h = cell_hash(params.tick, u32(x), u32(y), params.seed);
    var d = -1;
    if ((h & 1u) == 1u) { d = 1; }
    if (is_empty(x + d, y + 1)) { return lin(x + d, y + 1); }
    if (is_empty(x - d, y + 1)) { return lin(x - d, y + 1); }
    if (SIDEWAYS) {
        var s = -1;
        if (((h >> 1u) & 1u) == 1u) { s = 1; }
        if (is_empty(x + s, y)) { return lin(x + s, y); }
        if (is_empty(x - s, y)) { return lin(x - s, y); }
    }
    return -1;
}

fn plan(x: i32, y: i32) -> Plan {
    var p = Plan(false, -1, 0, 0, 0, 0);
    if (y < 0 || (u32(y) & 1u) != params.phase || mat_at(x, y) != MOVER) {
        return p;
    }
    let c = src[lin(x, y)];
    if (c.flags == params.tick + 1u) {
        return p;
    }
    p.live = true;
    let v = clamp(i32(c.vy) + GRAVITY_FP, 0, TERMINAL_FP);
    let acc = clamp(i32(c.vx), 0, FP_ONE - 1) + v;
    let n = acc / FP_ONE;
    let kept_v = v;
    let kept_f = acc % FP_ONE;
    if (n == 0) {
        p.mv = kept_v; p.mf = kept_f; p.sv = kept_v; p.sf = kept_f;
        return p;
    }
    var m = 0;
    while (m < n && is_empty(x, y + 1 + m)) {
        m += 1;
    }
    if (m > 0) {
        p.dest = lin(x, y + m);
        if (m == n) { p.mv = kept_v; p.mf = kept_f; }
    } else {
        p.dest = slide(x, y);
    }
    return p;
}

// Source cell that wins the empty cell (x, y): the nearest mover straight
// above (clear path), above-left, above-right, left, right. -1 when none.
fn winner(x: i32, y: i32) -> i32 {
    let dst_i = lin(x, y);
    for (var d = 1; d <= MAX_STEP; d += 1) {
        if (!is_empty(x, y - d)) {
            if (plan(x, y - d).dest == dst_i) { return lin(x, y - d); }
            break;
        }
    }
    if (plan(x - 1, y - 1).dest == dst_i) { return lin(x - 1, y - 1); }
    if (plan(x + 1, y - 1).dest == dst_i) { return lin(x + 1, y - 1); }
    if (plan(x - 1, y).dest == dst_i) { return lin(x - 1, y); }
    if (plan(x + 1, y).dest == dst_i) { return lin(x + 1, y); }
    return -1;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.w || gid.y >= params.h) {
        return;
    }
    let x = i32(gid.x);
    let y = i32(gid.y);
    let i = lin(x, y);
    var out = src[i];
    if (out.mat == EMPTY) {
        let s = winner(x, y);
        if (s >= 0) {
            let p = plan(s % i32(params.w), s / i32(params.w));
            out = src[s];
            out.vy = f32(p.mv);
            out.vx = f32(p.mf);
            out.flags = params.tick + 1u;
        }
    } else {
        let p = plan(x, y);
        if (p.live) {
            if (p.dest >= 0 && winner(p.dest % i32(params.w), p.dest / i32(params.w)) == i) {
                out = Cell();
            } else {
                out.vy = f32(p.sv);
                out.vx = f32(p.sf);
                out.flags = params.tick + 1u;
            }
        }
    }
    dst[i] = out;
}
