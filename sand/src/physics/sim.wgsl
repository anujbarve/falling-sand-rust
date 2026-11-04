// GPU copy of the rules in mod.rs (`heat_cell` and `move_block`).
// Every number and every branch must match the Rust version exactly:
// the tests `gpu_gives_identical_world_to_cpu` and
// `gpu_matches_cpu_on_odd_sizes` check that they do.
//
// One step is ONE dispatch (`step`, at the bottom): the heat pass and the
// move pass are fused. Each thread owns one 2x2 block of this step's block
// grid: it reads the block and its neighbours from the old grid, computes
// the 4 heat results, moves the block in registers and writes its 4 cells
// to the new grid.
//
// Chunk sleeping (rule and exactness proof: "Chunk sleeping" in mod.rs):
// a cell in a sleeping 32x32 chunk is not recomputed. Its heat result is
// provably its old value, and cells_out already holds that value, so
// the thread uses the old value and does not write it. A block is moved
// only if its owner chunk (the one holding its top-left cell) is awake. A
// chunk is active when heat changed one of its cells or a block it owns
// could move; the small `wake` pass turns that into next step's awake flags.

struct Params {
    width: u32,
    height: u32,
    frame: u32,
    _pad: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> cells_in: array<u32>;
@group(0) @binding(2) var<storage, read_write> cells_out: array<u32>;
// Per chunk: the last frame in which it was active ...
@group(0) @binding(3) var<storage, read_write> chunk_active: array<atomic<u32>>;
// ... and whether it runs in the current step (written by `wake`).
@group(0) @binding(4) var<storage, read_write> awake: array<u32>;

const CHUNK: u32 = 32u; // = CHUNK in mod.rs
fn chunks_x() -> u32 { return (params.width + CHUNK - 1u) / CHUNK; }
fn chunks_y() -> u32 { return (params.height + CHUNK - 1u) / CHUNK; }
fn chunk_of(x: u32, y: u32) -> u32 { return (y / CHUNK) * chunks_x() + x / CHUNK; }
fn mark_active(chunk: u32) {
    // Plain check first: most active chunks are already stamped this frame,
    // and a load is cheaper than contended atomic stores.
    if (atomicLoad(&chunk_active[chunk]) != params.frame) {
        atomicStore(&chunk_active[chunk], params.frame);
    }
}

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

// The material tables of mod.rs, packed into constants. (A `var<private>`
// array would be copied into every thread's local memory, and indexing it
// costs memory accesses; shifting a constant is a couple of ALU ops.)
//
// KIND: 2 bits per material.
const KIND_BITS: u32 = (GAS << 0u) | (SOLID << 2u) | (POWDER << 4u) | (LIQUID << 6u) | (LIQUID << 8u)
    | (LIQUID << 10u) | (GAS << 12u) | (SOLID << 14u) | (GAS << 16u) | (SOLID << 18u) | (GAS << 20u);
fn kind(m: u32) -> u32 { return (KIND_BITS >> (2u * m)) & 3u; }

// Byte tables: 4 materials per u32 (lowest byte first): materials 0-3 in
// x, 4-7 in y, 8-10 in z.
fn table_byte(t: vec3<u32>, m: u32) -> u32 {
    let word = select(select(t.x, t.y, m >= 4u), t.z, m >= 8u);
    return (word >> ((m & 3u) * 8u)) & 0xFFu;
}
const DENSITY_T = vec3<u32>(
    10u | (255u << 8u) | (150u << 16u) | (100u << 24u), // air stone sand water
    80u | (250u << 8u) | (3u << 16u) | (255u << 24u),   // oil lava steam wood
    9u | (255u << 8u) | (6u << 16u),                    // fire ice smoke
);
const COND_T = vec3<u32>(
    15u | (100u << 8u) | (40u << 16u) | (120u << 24u),
    40u | (80u << 8u) | (20u << 16u) | (40u << 24u),
    80u | (150u << 8u) | (15u << 16u),
);
// FLOW of air is 256 in mod.rs, stored here as 255. That gives identical
// results: flow() is only compared against a roll for powders and liquids,
// and in spreads() as min(flow(left), flow(right)) of two DIFFERENT
// materials, so at most one of them is air and the min is at most 230.
const FLOW_T = vec3<u32>(
    255u | (0u << 8u) | (200u << 16u) | (230u << 24u),
    150u | (40u << 8u) | (200u << 16u) | (0u << 24u),
    120u | (0u << 8u) | (150u << 16u),
);
fn base_density(m: u32) -> i32 { return i32(table_byte(DENSITY_T, m)); }
fn cond(m: u32) -> i32 { return i32(table_byte(COND_T, m)); }
fn flow(m: u32) -> u32 { return table_byte(FLOW_T, m); }

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

// Threads per workgroup; each thread owns one 2x2 block, so a workgroup
// covers 2*GROUP_W x 2*GROUP_H cells. gpu.rs reads these two lines.
const GROUP_W: u32 = 16u;
const GROUP_H: u32 = 16u;

// ---------------------------------------------------------------------------
// Heat: one cell.
// ---------------------------------------------------------------------------

// Heat for cell (x, y) given its old value and its 4 old neighbours
// (a neighbour outside the grid is ignored, whatever value is passed).
fn heat(me: u32, left: u32, right: u32, up: u32, down: u32, x: u32, y: u32, w: u32, h: u32, frame: u32) -> u32 {
    let m = material(me);
    var life = aux(me);
    let t0 = temp(me);
    let k_self = cond(m);

    // Neighbours left, right, up, down, as in heat_cell. A neighbour outside
    // the grid does not exist (no flux), and the bottom edge supports.
    var flux = 0;
    var touches_water = false;
    if (x > 0u) {
        let other = left;
        let mn = material(other);
        flux += (temp(other) - t0) * min(k_self, cond(mn)) / 1024;
        touches_water = touches_water || mn == WATER;
    }
    if (x + 1u < w) {
        let other = right;
        let mn = material(other);
        flux += (temp(other) - t0) * min(k_self, cond(mn)) / 1024;
        touches_water = touches_water || mn == WATER;
    }
    if (y > 0u) {
        let other = up;
        let mn = material(other);
        flux += (temp(other) - t0) * min(k_self, cond(mn)) / 1024;
        touches_water = touches_water || mn == WATER;
    }
    var supported = true;
    if (y + 1u < h) {
        let other = down;
        let mn = material(other);
        flux += (temp(other) - t0) * min(k_self, cond(mn)) / 1024;
        touches_water = touches_water || mn == WATER;
        supported = kind(mn) >= POWDER;
    }
    var t = t0 + flux;

    if (kind(m) == GAS && m != FIRE) {
        t += (AMBIENT - t) / 64;
    }

    // heat_cell's random number `r`, computed only in the branches that use it.
    let salt = frame ^ 0xA511E9B3u;
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
        if (t >= celsius(300)) { next = FIRE; life = 40u + (hash3(x, y, salt) & 63u); }
    } else if (m == OIL) {
        if (t >= celsius(250)) { next = FIRE; life = 20u + (hash3(x, y, salt) & 31u); }
    } else if (m == FIRE) {
        if (touches_water || life == 0u) {
            next = SMOKE;
            life = 30u + (hash3(x, y, salt) & 31u);
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
// Movement: one 2x2 block, in registers.
// ---------------------------------------------------------------------------

fn density(cell: u32) -> i32 {
    let m = material(cell);
    let d = base_density(m);
    if (kind(m) == GAS && m != FIRE) {
        return max(d - (temp(cell) - AMBIENT) / celsius(40), 1);
    }
    return d;
}

fn sinks(top: u32, bottom: u32, roll: u32) -> bool {
    let mt = material(top);
    let mb = material(bottom);
    if (kind(mt) == SOLID || kind(mb) == SOLID || kind(mb) == POWDER) {
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
    return ml != mr && is_fluid(kind(ml)) && is_fluid(kind(mr)) && roll < min(flow(ml), flow(mr));
}

// Can move_block change this block at all? Not if all 4 cells are one
// material whose density is fixed (no density gap, and spreading needs two
// different materials), nor if all 4 cells are identical (same gas at the
// same temperature). Skipping those saves the hash and every branch.
fn may_move(c: array<u32, 4>) -> bool {
    let m = material(c[0]);
    if (material(c[1]) != m || material(c[2]) != m || material(c[3]) != m) {
        return true;
    }
    if (kind(m) != GAS || m == FIRE) {
        return false;
    }
    return !(c[1] == c[0] && c[2] == c[0] && c[3] == c[0]);
}

// Could the block change for SOME random number? Copy of `block_can_move`
// in mod.rs: this decides whether the owner chunk counts as active.
fn block_can_move(c: array<u32, 4>) -> bool {
    var diag = false;
    for (var col = 0u; col < 2u; col += 1u) {
        let m = material(c[col]);
        if ((kind(m) == POWDER || kind(m) == LIQUID) && flow(m) > 0u && sinks(c[col], c[3u - col], 0u)) {
            diag = true;
        }
    }
    return sinks(c[0], c[2], 0u) || sinks(c[1], c[3], 0u) || diag
        || spreads(c[0], c[1], 0u) || spreads(c[2], c[3], 0u);
}

fn move_block(cells: ptr<function, array<u32, 4>>, r: u32) {
    let r2 = pcg(r);
    var fell = array<bool, 2>(false, false);

    for (var col = 0u; col < 2u; col += 1u) {
        let top = col;
        let bottom = col + 2u;
        if (sinks((*cells)[top], (*cells)[bottom], (r >> (col * 8u)) & 255u)) {
            let c = (*cells)[top]; (*cells)[top] = (*cells)[bottom]; (*cells)[bottom] = c;
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
        let m = material((*cells)[top]);
        if (kind(m) != POWDER && kind(m) != LIQUID) {
            continue;
        }
        let roll = (r >> (16u + col * 8u)) & 255u;
        if (roll < flow(m) && sinks((*cells)[top], (*cells)[diagonal], 0u)) {
            let c = (*cells)[top]; (*cells)[top] = (*cells)[diagonal]; (*cells)[diagonal] = c;
            slid = true;
        }
    }

    if (!fell[0] && !fell[1] && !slid) {
        for (var row = 0u; row < 2u; row += 1u) {
            let left = row * 2u;
            let right = row * 2u + 1u;
            if (spreads((*cells)[left], (*cells)[right], (r2 >> (row * 8u)) & 255u)) {
                let c = (*cells)[left]; (*cells)[left] = (*cells)[right]; (*cells)[right] = c;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The fused step: one thread per 2x2 block.
//
// Block origin offset o = frame & 1. Thread (tx, ty) owns grid cells
// (x, y) .. (x+1, y+1) with x = 2*tx - o, y = 2*ty - o, so its 4 cells are
// exactly block (bx, by) = ((x - o) / 2, (y - o) / 2) of this step. When
// o = 1 the first thread column/row starts at -1: its "block" hangs off the
// edge, holding only column 0 / row 0. Such partial blocks (also the last
// column / row when the size's parity leaves it out of the block grid) are
// not moved, but each of their cells inside the grid still gets its heat
// result written. Threads cover the plane without overlap, so every grid
// cell is written exactly once.
//
// Heat reads only the old grid (cells_in) and the move only touches the
// thread's own 4 cells, so this is exactly "heat everything, then move
// everything". Neighbours are read straight from cells_in: they are shared
// with adjacent threads and hit the cache. (Staging a tile plus halo in
// workgroup memory was measured 20-25% slower on Apple M5.)
// ---------------------------------------------------------------------------

// Read cell (gx, gy) of the old grid; 0 if outside (never used then).
fn load(gx: i32, gy: i32, w: u32, h: u32) -> u32 {
    if (gx >= 0 && gy >= 0 && gx < i32(w) && gy < i32(h)) {
        return cells_in[u32(gy) * w + u32(gx)];
    }
    return 0u;
}

@compute @workgroup_size(GROUP_W, GROUP_H)
fn step(@builtin(global_invocation_id) id: vec3<u32>) {
    let w = params.width;
    let h = params.height;
    let frame = params.frame;
    let o = frame & 1u;
    let x = 2 * i32(id.x) - i32(o); // signed: can be -1
    let y = 2 * i32(id.y) - i32(o);
    // Unsigned copies; only used where the cell is inside the grid.
    let ux = u32(x);
    let uy = u32(y);

    // Which of the 4 cells are inside the grid, and the chunks they are in.
    let in_x = array<bool, 2>(x >= 0 && x < i32(w), x + 1 < i32(w));
    let in_y = array<bool, 2>(y >= 0 && y < i32(h), y + 1 < i32(h));
    var chunk: array<u32, 4>;
    var wake = array<bool, 4>(false, false, false, false);
    var any_awake = false;
    for (var k = 0u; k < 4u; k += 1u) {
        let dx = k & 1u;
        let dy = k >> 1u;
        if (in_x[dx] && in_y[dy]) {
            chunk[k] = chunk_of(ux + dx, uy + dy);
            wake[k] = awake[chunk[k]] != 0u;
            any_awake = any_awake || wake[k];
        }
    }
    // Everything this thread owns is asleep: cells_out already holds it.
    if (!any_awake) {
        return;
    }

    // The block (c..) and its 8 edge neighbours: a = above, l = left,
    // r = right, b = below. Corners are not needed by heat.
    var a0: u32; var a1: u32; var l0: u32; var c00: u32; var c01: u32; var r0: u32;
    var l1: u32; var c10: u32; var c11: u32; var r1: u32; var b0: u32; var b1: u32;
    // Fast path: away from the grid edges no bounds checks are needed.
    let interior = x >= 1 && y >= 1 && x + 2 < i32(w) && y + 2 < i32(h);
    if (interior) {
        let i = uy * w + ux;
        a0 = cells_in[i - w]; a1 = cells_in[i - w + 1u];
        l0 = cells_in[i - 1u]; c00 = cells_in[i]; c01 = cells_in[i + 1u]; r0 = cells_in[i + 2u];
        let j = i + w;
        l1 = cells_in[j - 1u]; c10 = cells_in[j]; c11 = cells_in[j + 1u]; r1 = cells_in[j + 2u];
        b0 = cells_in[j + w]; b1 = cells_in[j + w + 1u];

        // Fast path: block and neighbours all one identical value v. Then
        // all 4 heat results are identical (position only matters at grid
        // edges, excluded here, and in the hash, which only wood, oil and
        // fire use), so compute it once; 4 identical cells never move.
        let v = c00;
        let m = material(v);
        let same = a0 == v && a1 == v && l0 == v && c01 == v && r0 == v && l1 == v
            && c10 == v && c11 == v && r1 == v && b0 == v && b1 == v;
        if (same && m != WOOD && m != OIL && m != FIRE) {
            let nv = heat(v, v, v, v, v, ux, uy, w, h, frame);
            let idx = array<u32, 4>(i, i + 1u, j, j + 1u);
            for (var k = 0u; k < 4u; k += 1u) {
                if (wake[k]) {
                    cells_out[idx[k]] = nv;
                    if (nv != v) {
                        mark_active(chunk[k]);
                    }
                }
            }
            return;
        }
    } else {
        a0 = load(x, y - 1, w, h);
        a1 = load(x + 1, y - 1, w, h);
        l0 = load(x - 1, y, w, h);
        c00 = load(x, y, w, h);
        c01 = load(x + 1, y, w, h);
        r0 = load(x + 2, y, w, h);
        l1 = load(x - 1, y + 1, w, h);
        c10 = load(x, y + 1, w, h);
        c11 = load(x + 1, y + 1, w, h);
        r1 = load(x + 2, y + 1, w, h);
        b0 = load(x, y + 2, w, h);
        b1 = load(x + 1, y + 2, w, h);
    }

    // 1. Heat, for each of the 4 cells that is inside the grid and awake.
    //    A sleeping cell keeps its old value (that IS its heat result).
    let old = array<u32, 4>(c00, c01, c10, c11);
    var cells = old;
    if (wake[0]) { cells[0] = heat(c00, l0, c01, a0, c10, ux, uy, w, h, frame); }
    if (wake[1]) { cells[1] = heat(c01, c00, r0, a1, c11, ux + 1u, uy, w, h, frame); }
    if (wake[2]) { cells[2] = heat(c10, l1, c11, c00, b0, ux, uy + 1u, w, h, frame); }
    if (wake[3]) { cells[3] = heat(c11, c10, r1, c01, b1, ux + 1u, uy + 1u, w, h, frame); }
    var write = wake;
    for (var k = 0u; k < 4u; k += 1u) {
        if (wake[k] && cells[k] != old[k]) {
            mark_active(chunk[k]);
        }
    }

    // 2. Move, if the whole block is inside the grid (then x - o, y - o >= 0)
    //    and its owner chunk (top-left cell) is awake.
    let whole = x >= 0 && y >= 0 && in_x[1] && in_y[1];
    if (whole && wake[0] && may_move(cells) && block_can_move(cells)) {
        mark_active(chunk[0]);
        move_block(&cells, hash3((ux - o) / 2u, (uy - o) / 2u, frame));
        write = array<bool, 4>(true, true, true, true);
    }

    // 3. Write the new grid.
    for (var k = 0u; k < 4u; k += 1u) {
        let dx = k & 1u;
        let dy = k >> 1u;
        if (write[k]) {
            cells_out[(uy + dy) * w + ux + dx] = cells[k];
        }
    }
}

// ---------------------------------------------------------------------------
// Wake. One thread per chunk, after the step of frame `frame`. A chunk runs
// in step frame+1 if it or a neighbour was active in step frame or frame-1.
// (Wrapping u32 maths, like the CPU's `is_awake`.)
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
