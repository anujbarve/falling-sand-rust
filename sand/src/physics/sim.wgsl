// GPU copy of the rules in mod.rs (`heat_cell` and `move_block`).
// Every number and every branch must match the Rust version exactly:
// the test `gpu_gives_identical_world_to_cpu` checks that they do.

struct Params {
    width: u32,
    height: u32,
    frame: u32,
    _pad: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> cells_in: array<u32>;
@group(0) @binding(2) var<storage, read_write> cells_out: array<u32>;
// Chunk sleeping (see "Chunk sleeping" in mod.rs for the rule and why it is
// exact). Per chunk: the last frame in which it was active ...
@group(0) @binding(3) var<storage, read_write> chunk_active: array<atomic<u32>>;
// ... and whether it runs in the current step (written by `wake`).
@group(0) @binding(4) var<storage, read_write> awake: array<u32>;

const CHUNK: u32 = 32u; // = CHUNK in mod.rs
fn chunks_x() -> u32 { return (params.width + CHUNK - 1u) / CHUNK; }
fn chunks_y() -> u32 { return (params.height + CHUNK - 1u) / CHUNK; }

// Every workgroup belongs to exactly one chunk. Thread 0 reads the chunk's
// awake flag into `wg_awake`; `workgroupUniformLoad` hands it to all threads
// as a uniform value, so the whole workgroup can return early together.
// `wg_active` collects "something here is active" so that only one thread
// per workgroup touches the global `chunk_active` stamp.
var<workgroup> wg_awake: u32;
var<workgroup> wg_active: atomic<u32>;

const AIR: u32 = 0u;
const STONE: u32 = 1u;
const SAND: u32 = 2u;
const WATER: u32 = 3u;
const OIL: u32 = 4u;
const LAVA: u32 = 5u;
const STEAM: u32 = 6u;
const WOOD: u32 = 7u;
const FIRE: u32 = 8u;
const ICE: u32 = 9u;
const SMOKE: u32 = 10u;

const GAS: u32 = 0u;
const LIQUID: u32 = 1u;
const POWDER: u32 = 2u;
const SOLID: u32 = 3u;

const T_SCALE: i32 = 256;
const AMBIENT: i32 = 5120; // 20°C
const T_MIN: i32 = -524288; // -(1 << 19)
const T_MAX: i32 = 524287;  //  (1 << 19) - 1

// `var<private>` (not `const`) so they can be indexed with a runtime value.
var<private> KIND: array<u32, 11> = array<u32, 11>(GAS, SOLID, POWDER, LIQUID, LIQUID, LIQUID, GAS, SOLID, GAS, SOLID, GAS);
var<private> DENSITY: array<i32, 11> = array<i32, 11>(10, 255, 150, 100, 80, 250, 3, 255, 9, 255, 6);
var<private> FLOW: array<u32, 11> = array<u32, 11>(256u, 0u, 200u, 230u, 150u, 40u, 200u, 0u, 120u, 0u, 150u);
var<private> COND: array<i32, 11> = array<i32, 11>(15, 100, 40, 120, 40, 80, 20, 40, 80, 150, 15);

// Cell layout: bits 0-3 material, 4-11 aux, 12-31 signed temperature.
fn celsius(c: i32) -> i32 { return c * T_SCALE; }
fn material(cell: u32) -> u32 { return cell & 0xFu; }
fn aux(cell: u32) -> u32 { return (cell >> 4u) & 0xFFu; }
// >> on an i32 is an arithmetic shift: it keeps the sign.
fn temp(cell: u32) -> i32 { return bitcast<i32>(cell) >> 12u; }
fn pack(m: u32, a: u32, t: i32) -> u32 {
    return m | (a << 4u) | (bitcast<u32>(clamp(t, T_MIN, T_MAX)) << 12u);
}

// u32 arithmetic in WGSL wraps around, like wrapping_mul / wrapping_add in Rust.
fn pcg(v: u32) -> u32 {
    let state = v * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn hash3(a: u32, b: u32, c: u32) -> u32 {
    return pcg(a ^ pcg(b ^ pcg(c)));
}

// ---------------------------------------------------------------------------
// Pass 1: heat. One thread per cell, old grid in, new grid out.
// ---------------------------------------------------------------------------

// A 16x16 heat workgroup is a quarter of a 32x32 chunk.
@compute @workgroup_size(16, 16)
fn heat(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(workgroup_id) wg: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
) {
    let chunk = (wg.y / 2u) * chunks_x() + wg.x / 2u;
    if (li == 0u) {
        wg_awake = awake[chunk];
        atomicStore(&wg_active, 0u);
    }
    if (workgroupUniformLoad(&wg_awake) == 0u) {
        // Asleep: cells_out already holds these cells' values (mod.rs, point 4).
        return;
    }
    if (id.x < params.width && id.y < params.height) {
        let i = id.y * params.width + id.x;
        let old = cells_in[i];
        let new_cell = heat_cell(id.x, id.y, old);
        cells_out[i] = new_cell;
        if (new_cell != old) {
            atomicStore(&wg_active, 1u);
        }
    }
    workgroupBarrier();
    if (li == 0u && atomicLoad(&wg_active) != 0u) {
        atomicStore(&chunk_active[chunk], params.frame);
    }
}

fn heat_cell(x: u32, y: u32, me: u32) -> u32 {
    let w = params.width;
    let h = params.height;
    let i = y * w + x;
    let m = material(me);
    var life = aux(me);
    let t0 = temp(me);
    let k_self = COND[m];

    var flux = 0;
    var touches_water = false;
    var js: array<u32, 4>;
    var n = 0u;
    if (x > 0u) { js[n] = i - 1u; n += 1u; }
    if (x + 1u < w) { js[n] = i + 1u; n += 1u; }
    if (y > 0u) { js[n] = i - w; n += 1u; }
    if (y + 1u < h) { js[n] = i + w; n += 1u; }
    for (var k = 0u; k < n; k += 1u) {
        let other = cells_in[js[k]];
        let mn = material(other);
        let c = min(k_self, COND[mn]);
        flux += (temp(other) - t0) * c / 1024;
        touches_water = touches_water || mn == WATER;
    }
    var t = t0 + flux;
    var supported = true;
    if (y + 1u < h) {
        supported = KIND[material(cells_in[i + w])] >= POWDER;
    }

    if (KIND[m] == GAS && m != FIRE) {
        t += (AMBIENT - t) / 64;
    }

    let r = hash3(x, y, params.frame ^ 0xA511E9B3u);
    var next = m;
    if (m == WATER) {
        if (t >= celsius(100)) { next = STEAM; } else if (t <= celsius(-2)) { next = ICE; }
    } else if (m == ICE) {
        if (t >= celsius(1)) { next = WATER; }
    } else if (m == STEAM) {
        if (t < celsius(50)) { next = WATER; }
    } else if (m == LAVA) {
        if (t < celsius(800)) { next = select(SAND, STONE, supported); }
    } else if (m == STONE) {
        if (t >= celsius(1100)) { next = LAVA; }
    } else if (m == WOOD) {
        if (t >= celsius(300)) { next = FIRE; life = 40u + (r & 63u); }
    } else if (m == OIL) {
        if (t >= celsius(250)) { next = FIRE; life = 20u + (r & 31u); }
    } else if (m == FIRE) {
        if (touches_water || life == 0u) {
            next = SMOKE;
            life = 30u + (r & 31u);
        } else {
            life -= 1u;
            t = max(t, celsius(900));
        }
    } else if (m == SMOKE) {
        if (life == 0u) { next = AIR; } else { life -= 1u; }
    }
    if (next == AIR) {
        life = 0u;
    }
    return pack(next, life, t);
}

// ---------------------------------------------------------------------------
// Pass 2: movement. One thread per 2x2 block, in place on the new grid.
// ---------------------------------------------------------------------------

fn density(cell: u32) -> i32 {
    let m = material(cell);
    let d = DENSITY[m];
    if (KIND[m] == GAS && m != FIRE) {
        return max(d - (temp(cell) - AMBIENT) / celsius(40), 1);
    }
    return d;
}

fn sinks(top: u32, bottom: u32, roll: u32) -> bool {
    let mt = material(top);
    let mb = material(bottom);
    if (KIND[mt] == SOLID || KIND[mb] == SOLID || KIND[mb] == POWDER) {
        return false;
    }
    let dt = density(top);
    let db = density(bottom);
    if (dt <= db) {
        return false;
    }
    return roll < u32((dt - db) * 256 / dt);
}

fn is_fluid(k: u32) -> bool {
    return k == GAS || k == LIQUID;
}

fn spreads(left: u32, right: u32, roll: u32) -> bool {
    let ml = material(left);
    let mr = material(right);
    return ml != mr && is_fluid(KIND[ml]) && is_fluid(KIND[mr]) && roll < min(FLOW[ml], FLOW[mr]);
}

// Could the block change for SOME random number? Copy of `block_can_move`.
fn block_can_move(c: array<u32, 4>) -> bool {
    var diag = false;
    for (var col = 0u; col < 2u; col += 1u) {
        let m = material(c[col]);
        if ((KIND[m] == POWDER || KIND[m] == LIQUID) && FLOW[m] > 0u && sinks(c[col], c[3u - col], 0u)) {
            diag = true;
        }
    }
    return sinks(c[0], c[2], 0u) || sinks(c[1], c[3], 0u) || diag
        || spreads(c[0], c[1], 0u) || spreads(c[2], c[3], 0u);
}

// A 16x16 workgroup of 2x2 blocks covers exactly one chunk: block (bx, by)
// has its top-left cell at (2bx + o, 2by + o), which is in chunk
// (bx / 16, by / 16) for both offsets. That chunk owns the block.
@compute @workgroup_size(16, 16)
fn move_blocks(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(workgroup_id) wg: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
) {
    let chunk = wg.y * chunks_x() + wg.x;
    if (li == 0u) {
        wg_awake = awake[chunk];
        atomicStore(&wg_active, 0u);
    }
    if (workgroupUniformLoad(&wg_awake) == 0u) {
        return;
    }
    if (move_block(id.x, id.y)) {
        atomicStore(&wg_active, 1u);
    }
    workgroupBarrier();
    if (li == 0u && atomicLoad(&wg_active) != 0u) {
        atomicStore(&chunk_active[chunk], params.frame);
    }
}

// Returns whether the block could move (that makes its chunk active).
fn move_block(bx: u32, by: u32) -> bool {
    let w = params.width;
    let h = params.height;
    if (bx >= w / 2u || by >= h / 2u) {
        return false;
    }
    let o = params.frame & 1u;
    let x = 2u * bx + o;
    let y = 2u * by + o;
    if (x + 1u >= w || y + 1u >= h) {
        return false;
    }
    let idx = array<u32, 4>(y * w + x, y * w + x + 1u, (y + 1u) * w + x, (y + 1u) * w + x + 1u);
    var cells: array<u32, 4>;
    for (var k = 0u; k < 4u; k += 1u) {
        cells[k] = cells_out[idx[k]];
    }
    // Cannot move for any random number: nothing to do, nothing to write.
    if (!block_can_move(cells)) {
        return false;
    }

    let r = hash3(bx, by, params.frame);
    let r2 = pcg(r);
    var fell = array<bool, 2>(false, false);

    for (var col = 0u; col < 2u; col += 1u) {
        let top = col;
        let bottom = col + 2u;
        if (sinks(cells[top], cells[bottom], (r >> (col * 8u)) & 255u)) {
            let c = cells[top]; cells[top] = cells[bottom]; cells[bottom] = c;
            fell[col] = true;
        }
    }

    var slid = false;
    for (var col = 0u; col < 2u; col += 1u) {
        if (fell[col]) {
            continue;
        }
        let top = col;
        let diagonal = 3u - col;
        let m = material(cells[top]);
        if (KIND[m] != POWDER && KIND[m] != LIQUID) {
            continue;
        }
        let roll = (r >> (16u + col * 8u)) & 255u;
        if (roll < FLOW[m] && sinks(cells[top], cells[diagonal], 0u)) {
            let c = cells[top]; cells[top] = cells[diagonal]; cells[diagonal] = c;
            slid = true;
        }
    }

    if (!fell[0] && !fell[1] && !slid) {
        for (var row = 0u; row < 2u; row += 1u) {
            let left = row * 2u;
            let right = row * 2u + 1u;
            if (spreads(cells[left], cells[right], (r2 >> (row * 8u)) & 255u)) {
                let c = cells[left]; cells[left] = cells[right]; cells[right] = c;
            }
        }
    }

    for (var k = 0u; k < 4u; k += 1u) {
        cells_out[idx[k]] = cells[k];
    }
    return true;
}

// ---------------------------------------------------------------------------
// Pass 3: wake. One thread per chunk, after the move pass of step `frame`.
// A chunk runs in step frame+1 if it or a neighbour was active in step
// frame or frame-1. (Wrapping u32 maths, like the CPU's `is_awake`.)
// ---------------------------------------------------------------------------

@compute @workgroup_size(8, 8)
fn wake(@builtin(global_invocation_id) id: vec3<u32>) {
    let cw = chunks_x();
    let ch = chunks_y();
    if (id.x >= cw || id.y >= ch) {
        return;
    }
    var a = 0u;
    for (var ny = max(id.y, 1u) - 1u; ny < min(id.y + 2u, ch); ny += 1u) {
        for (var nx = max(id.x, 1u) - 1u; nx < min(id.x + 2u, cw); nx += 1u) {
            if (params.frame - atomicLoad(&chunk_active[ny * cw + nx]) < 2u) {
                a = 1u;
            }
        }
    }
    awake[id.y * cw + id.x] = a;
}
