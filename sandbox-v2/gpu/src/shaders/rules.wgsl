// Shared row-parity kernel. Concatenated after common.wgsl and before
// granular.wgsl / liquid.wgsl, which define `intent`. Mirrors src/reference.rs.
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

const EMPTY: u32 = 0u;
const SAND: u32 = 1u;
const WATER: u32 = 2u;
const WALL: u32 = 3u;

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

// Source cell that wins the empty cell (x, y): above, above-left,
// above-right, left, right. Returns -1 when nobody targets it.
fn winner(x: i32, y: i32) -> i32 {
    let tgt = lin(x, y);
    if (intent(x, y - 1) == tgt) { return lin(x, y - 1); }
    if (intent(x - 1, y - 1) == tgt) { return lin(x - 1, y - 1); }
    if (intent(x + 1, y - 1) == tgt) { return lin(x + 1, y - 1); }
    if (intent(x - 1, y) == tgt) { return lin(x - 1, y); }
    if (intent(x + 1, y) == tgt) { return lin(x + 1, y); }
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
            out = src[s];
        }
    } else {
        let t = intent(x, y);
        if (t >= 0 && winner(t % i32(params.w), t / i32(params.w)) == i) {
            out = Cell();
        }
    }
    dst[i] = out;
}
