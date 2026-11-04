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

// Real units: see "Units" in mod.rs. 1 cell = 5 mm, 1 step = 1/1200 s.
const G_Q: i32 = 89;          // gravity per step, 1/256 of a speed level
const GAS_RISE: i32 = 42;     // 1 m/s
const RELAX_K: i32 = 291;     // gases back to 20°C, 3 s time constant
const TICK_STEPS: u32 = 64u;  // fire/smoke life tick
const EDGE_SALT: u32 = 0x5EED1234u;
const CELL_SALT: u32 = 0xA511E9B3u;

// The material tables of mod.rs. Small ones are packed into constants
// (a `var<private>` array is copied into every thread's local memory, and
// indexing it costs memory accesses; shifting a constant is a few ALU ops).
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
// FLOW of air is 256 in mod.rs, stored here as 255. That gives identical
// results: flow() is only compared against a roll for powders and liquids,
// and in spread_chance() as the min of two DIFFERENT materials, so at most
// one of them is air and the min is at most 42.
const FLOW_T = vec3<u32>(
    255u | (0u << 8u) | (21u << 16u) | (42u << 24u), // air stone sand water
    17u | (2u << 8u) | (42u << 16u) | (0u << 24u),    // oil lava steam wood
    21u | (0u << 8u) | (12u << 16u),                  // fire ice smoke
);
const VT_LIQUID_T = vec3<u32>(
    0u | (0u << 8u) | (4u << 16u) | (4u << 24u),
    4u | (12u << 8u) | (10u << 16u) | (0u << 24u),
    0u,
);
const VT_GAS_T = vec3<u32>(
    0u | (0u << 8u) | (255u << 16u) | (255u << 24u),
    255u | (255u << 8u) | (85u << 16u) | (0u << 24u),
    0u,
);
// Top speed of anything moving through this medium (VT_MEDIUM in mod.rs):
// 255 for air, water, oil, steam, fire, smoke; 0 for lava and solids.
const VT_MEDIUM_T = vec3<u32>(
    255u | (0u << 8u) | (0u << 16u) | (255u << 24u),
    255u | (0u << 8u) | (255u << 16u) | (0u << 24u),
    255u | (0u << 8u) | (255u << 16u),
);
fn vt_medium(m: u32) -> u32 { return table_byte(VT_MEDIUM_T, m); }
fn flow(m: u32) -> u32 { return table_byte(FLOW_T, m); }
fn vt_liquid(m: u32) -> u32 { return table_byte(VT_LIQUID_T, m); }
fn vt_gas(m: u32) -> u32 { return table_byte(VT_GAS_T, m); }

fn base_density(m: u32) -> i32 {
    switch m {
        case 0u: { return 12; }
        case 1u: { return 26000; }
        case 2u: { return 16000; }
        case 3u: { return 10000; }
        case 4u: { return 9000; }
        case 5u: { return 26000; }
        case 6u: { return 8; }
        case 7u: { return 6000; }
        case 8u: { return 3; }
        case 9u: { return 9170; }
        default: { return 12; }
    }
}
// Thermal conductivity, mW/(m K). Copy of COND in mod.rs.
fn cond(m: u32) -> i32 {
    switch m {
        case 0u: { return 26; }
        case 1u: { return 2500; }
        case 2u: { return 300; }
        case 3u: { return 600; }
        case 4u: { return 150; }
        case 5u: { return 1500; }
        case 6u: { return 25; }
        case 7u: { return 150; }
        case 8u: { return 80; }
        case 9u: { return 2200; }
        default: { return 26; }
    }
}
// Heat capacity rho*c, 10 J/(m^3 K). Copy of CAP in mod.rs.
fn cap(m: u32) -> i32 {
    switch m {
        case 0u: { return 120; }
        case 1u: { return 210000; }
        case 2u: { return 130000; }
        case 3u: { return 418000; }
        case 4u: { return 170000; }
        case 5u: { return 300000; }
        case 6u: { return 120; }
        case 7u: { return 102000; }
        case 8u: { return 35; }
        case 9u: { return 190000; }
        default: { return 120; }
    }
}
// Liquid thermal expansion, ppm/K. Copy of EXPANSION in mod.rs.
fn expansion(m: u32) -> i32 {
    switch m {
        case 3u: { return 300; }
        case 4u: { return 700; }
        case 5u: { return 100; }
        default: { return 0; }
    }
}
const RATE_SCALE: i32 = 55924;
const RATE_MAX: i32 = 262143; // (1 << 18) - 1

// Cell layout: bits 0-3 material, 4-11 aux, 12-31 signed temperature.
fn celsius(c: i32) -> i32 { return c * T_SCALE; }
fn material(cell: u32) -> u32 { return cell & 0xFu; }
fn aux(cell: u32) -> u32 { return (cell >> 4u) & 0xFFu; }
// >> on an i32 is an arithmetic shift: it keeps the sign.
fn temp(cell: u32) -> i32 { return bitcast<i32>(cell) >> 12u; }
fn pack(m: u32, a: u32, t: i32) -> u32 {
    return m | (a << 4u) | (bitcast<u32>(clamp(t, T_MIN, T_MAX)) << 12u);
}
fn has_speed(m: u32) -> bool {
    return m == SAND || m == WATER || m == OIL || m == LAVA || m == STEAM;
}
fn sinker(m: u32) -> bool { return base_density(m) > 12; }
fn speed(cell: u32) -> u32 { return select(0u, aux(cell), has_speed(material(cell))); }
fn with_aux(cell: u32, a: u32) -> u32 { return (cell & ~(0xFFu << 4u)) | (a << 4u); }

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
// Heat, gravity on speed, phase changes: one cell. Copy of `heat_cell`.
// ---------------------------------------------------------------------------

fn edge_roll(i: u32, down: u32, key: u32) -> i32 {
    return i32(pcg((2u * i + down) ^ key) & 0xFFFFFFu);
}
// floor((d * rate + roll) / 2^24) without overflow. Copy of flow_24.
// (& on a negative i32 works on its two's complement bits, like Rust.)
fn flow_24(d: i32, rate: i32, roll: i32) -> i32 {
    let hi = d >> 12u;
    let lo = d & 4095;
    return (hi * rate + ((lo * rate + roll) >> 12u)) >> 12u;
}
fn condensed(m: u32) -> bool { return kind(m) != GAS; }
fn radiation_k(ta: i32, tb: i32) -> i32 {
    let kelvin = (ta + tb) / 2 / T_SCALE + 273;
    let t10 = max(kelvin / 10, 0);
    return (((t10 * t10 * t10) >> 6u) * 1045) >> 14u;
}
fn pair_conductance(a: u32, b: u32, ta: i32, tb: i32) -> i32 {
    let ka = cond(a);
    let kb = cond(b);
    let series = 2 * ka * kb / (ka + kb);
    var radiation = 0;
    if (a != b && condensed(a) && condensed(b)) {
        radiation = radiation_k(ta, tb);
    } else if ((a == FIRE && condensed(b)) || (b == FIRE && condensed(a))) {
        radiation = radiation_k(ta, tb) / 3;
    }
    return series + radiation;
}
fn heat_rate(m: u32, conductance: i32) -> i32 {
    return min(conductance * RATE_SCALE / cap(m), RATE_MAX);
}
fn sky_rate(m: u32, t: i32) -> i32 {
    let t10 = max((t / T_SCALE + 273) / 10, 0);
    let x = (t10 * t10 + 29 * 29) * (t10 + 29);
    return (x / 100) * 1427 / cap(m);
}

fn density(cell: u32) -> i32 {
    let m = material(cell);
    let d = base_density(m);
    let k = kind(m);
    if (k == GAS && m != FIRE) {
        return max(d * 293 * T_SCALE / max(temp(cell) + 273 * T_SCALE, 1), 1);
    }
    if (k == LIQUID) {
        let warmer = (temp(cell) - AMBIENT) / T_SCALE;
        return max(d - d * (expansion(m) * warmer / 1000) / 1000, 1);
    }
    return d;
}

fn sinks(top: u32, bottom: u32) -> bool {
    let kt = kind(material(top));
    let kb = kind(material(bottom));
    return kt != SOLID && kb != SOLID && kb != POWDER && density(top) > density(bottom);
}

fn accelerate(cell: u32, heavy: u32, light: u32, medium: u32, roll: u32) -> u32 {
    let dh = density(heavy);
    let dl = density(light);
    let q = u32(G_Q * (dh - dl) / dh);
    return min(speed(cell) + ((q + roll) >> 8u), speed_cap(cell, medium));
}

fn speed_cap(cell: u32, medium: u32) -> u32 {
    let m = material(cell);
    let mm = material(medium);
    let own = select(vt_gas(m), vt_liquid(m), kind(mm) == LIQUID);
    return min(own, vt_medium(mm));
}

fn blocked_speed(cell: u32, in_way: u32) -> u32 {
    let mw = material(in_way);
    let same_way = has_speed(mw) && sinker(mw) == sinker(material(cell));
    return min(speed(cell), select(0u, speed(in_way), same_way));
}

fn move_chance(s: u32) -> u32 { return 512u * s / (256u + s); }

struct HeatOut {
    cell: u32,
    maybe: bool, // could a random number have changed it? (chunk sleeping)
}

// Cell (x, y) given its old value and its 4 old neighbours (a neighbour
// outside the grid is ignored, whatever value is passed).
fn heat(me: u32, left: u32, right: u32, up: u32, down: u32, x: u32, y: u32, w: u32, h: u32, frame: u32) -> HeatOut {
    let m = material(me);
    var life = aux(me);
    let t0 = temp(me);
    var maybe = false;

    var t = t0;
    var touches_water = false;
    let i = y * w + x;
    let key = pcg(frame ^ EDGE_SALT);
    // Same as heat_cell: an edge with no temperature gap carries no heat
    // and needs no random number. The rate is OURS (our heat capacity).
    var open_faces = 0;
    if (x + 1u < w) {
        let mn = material(right);
        let tn = temp(right);
        touches_water = touches_water || mn == WATER;
        open_faces += select(0, 1, kind(mn) == GAS && mn != FIRE);
        if (tn != t0) {
            maybe = true;
            t += flow_24(tn - t0, heat_rate(m, pair_conductance(m, mn, t0, tn)), edge_roll(i, 0u, key));
        }
    }
    if (x > 0u) {
        let mn = material(left);
        let tn = temp(left);
        touches_water = touches_water || mn == WATER;
        open_faces += select(0, 1, kind(mn) == GAS && mn != FIRE);
        if (tn != t0) {
            maybe = true;
            t -= flow_24(t0 - tn, heat_rate(m, pair_conductance(m, mn, t0, tn)), edge_roll(i - 1u, 0u, key));
        }
    }
    if (y + 1u < h) {
        let mn = material(down);
        let tn = temp(down);
        touches_water = touches_water || mn == WATER;
        open_faces += select(0, 1, kind(mn) == GAS && mn != FIRE);
        if (tn != t0) {
            maybe = true;
            t += flow_24(tn - t0, heat_rate(m, pair_conductance(m, mn, t0, tn)), edge_roll(i, 1u, key));
        }
    }
    if (y > 0u) {
        let mn = material(up);
        let tn = temp(up);
        touches_water = touches_water || mn == WATER;
        open_faces += select(0, 1, kind(mn) == GAS && mn != FIRE);
        if (tn != t0) {
            maybe = true;
            t -= flow_24(t0 - tn, heat_rate(m, pair_conductance(m, mn, t0, tn)), edge_roll(i - w, 1u, key));
        }
    }

    let r = pcg(i ^ pcg(frame ^ CELL_SALT));
    let r2 = pcg(r);
    let tick = r2 % TICK_STEPS == 0u;
    if (condensed(m) && open_faces > 0 && t0 != AMBIENT) {
        maybe = true;
        t += flow_24(AMBIENT - t0, sky_rate(m, t0) * open_faces, i32(r2 >> 8u));
    }
    if (kind(m) == GAS && m != FIRE) {
        t += ((AMBIENT - t0) * RELAX_K + i32(r & 0xFFFFFu)) >> 20u;
        maybe = maybe || t0 != AMBIENT;
    }

    let has_below = y + 1u < h;
    let has_above = y > 0u;
    if (has_speed(m)) {
        let roll = (r >> 20u) & 255u;
        var free = false;
        var has_other = false;
        var other = 0u;
        if (sinker(m)) {
            has_other = has_below;
            other = down;
            free = has_below && sinks(me, down);
        } else {
            has_other = has_above;
            other = up;
            free = has_above && sinks(up, me);
        }
        if (free) {
            maybe = maybe || speed(me) < speed_cap(me, other);
            if (sinker(m)) {
                life = accelerate(me, me, other, other, roll);
            } else {
                life = accelerate(me, other, me, other, roll);
            }
        } else if (has_other) {
            life = blocked_speed(me, other);
        } else {
            life = 0u;
        }
    }

    var supported = true;
    if (has_below) {
        supported = kind(material(down)) >= POWDER;
    }
    var next = m;
    if (m == WATER) {
        if (t >= celsius(100)) { next = STEAM; } else if (t <= celsius(-2)) { next = ICE; }
    } else if (m == ICE) {
        if (t >= celsius(1)) { next = WATER; }
    } else if (m == STEAM) {
        if (t < celsius(50)) { next = WATER; }
    } else if (m == LAVA) {
        if (t < celsius(1000)) { next = select(SAND, STONE, supported); }
    } else if (m == STONE) {
        if (t >= celsius(1200)) { next = LAVA; }
    } else if (m == WOOD) {
        if (t >= celsius(300)) { next = FIRE; life = 120u + (r & 127u); }
    } else if (m == OIL) {
        if (t >= celsius(250)) { next = FIRE; life = 20u + (r & 31u); }
    } else if (m == FIRE) {
        maybe = true;
        if (touches_water || life == 0u) {
            next = SMOKE;
            life = 30u + (r & 31u);
        } else {
            if (tick) { life -= 1u; }
            t = max(t, celsius(900));
        }
    } else if (m == SMOKE) {
        maybe = true;
        if (life == 0u) { next = AIR; } else if (tick) { life -= 1u; }
    }
    if (next != m && next != FIRE && next != SMOKE) {
        life = 0u;
    }
    let new_cell = pack(next, life, t);
    return HeatOut(new_cell, maybe || new_cell != me);
}

// ---------------------------------------------------------------------------
// Movement: one 2x2 block, in registers. Copy of `move_block`.
// ---------------------------------------------------------------------------

fn pair_chance(top: u32, bottom: u32) -> u32 {
    if (!sinks(top, bottom)) {
        return 0u;
    }
    let body = select(bottom, top, sinker(material(top)));
    if (has_speed(material(body))) {
        return move_chance(speed(body));
    }
    let dt = density(top);
    let db = density(bottom);
    return move_chance(u32(GAS_RISE * (dt - db) / dt));
}

fn can_slide(top: u32, below: u32, diagonal: u32) -> bool {
    let m = material(top);
    let k = kind(m);
    return (k == POWDER || k == LIQUID) && flow(m) > 0u && speed(top) == 0u && !sinks(top, below) && sinks(top, diagonal);
}

fn is_fluid(k: u32) -> bool {
    return k == GAS || k == LIQUID;
}

fn spread_chance(left: u32, right: u32) -> u32 {
    let ml = material(left);
    let mr = material(right);
    if (ml != mr && is_fluid(kind(ml)) && is_fluid(kind(mr)) && speed(left) == 0u && speed(right) == 0u) {
        return min(flow(ml), flow(mr));
    }
    return 0u;
}

// Could the block change for SOME random number? Copy of `block_can_move`:
// this decides whether the owner chunk counts as active.
fn block_can_move(c: array<u32, 4>) -> bool {
    return pair_chance(c[0], c[2]) > 0u || pair_chance(c[1], c[3]) > 0u
        || can_slide(c[0], c[2], c[3]) || can_slide(c[1], c[3], c[2])
        || spread_chance(c[0], c[1]) > 0u || spread_chance(c[2], c[3]) > 0u;
}

// Cheap first filter: four cells of one material whose density is fixed
// (solids, powders, fire), or four identical cells, can never move
// (block_can_move would say no). Gases and liquids of one material can:
// their density depends on temperature, so hot under cold swaps.
fn may_move(c: array<u32, 4>) -> bool {
    let m = material(c[0]);
    if (material(c[1]) != m || material(c[2]) != m || material(c[3]) != m) {
        return true;
    }
    let k = kind(m);
    if (k == SOLID || k == POWDER || m == FIRE) {
        return false;
    }
    return !(c[1] == c[0] && c[2] == c[0] && c[3] == c[0]);
}

fn displace(cells: ptr<function, array<u32, 4>>, mover: u32, pushed: u32) {
    var p = (*cells)[pushed];
    if (has_speed(material(p))) {
        p = with_aux(p, 0u);
    }
    (*cells)[pushed] = (*cells)[mover];
    (*cells)[mover] = p;
}

fn move_block(cells: ptr<function, array<u32, 4>>, r: u32) {
    let r2 = pcg(r);
    var fell = array<bool, 2>(false, false);

    for (var col = 0u; col < 2u; col += 1u) {
        let top = col;
        let bottom = col + 2u;
        if (((r >> (col * 8u)) & 255u) < pair_chance((*cells)[top], (*cells)[bottom])) {
            if (sinker(material((*cells)[top]))) {
                displace(cells, top, bottom);
            } else {
                displace(cells, bottom, top);
            }
            fell[col] = true;
        }
    }

    var slid = false;
    for (var col = 0u; col < 2u; col += 1u) {
        if (fell[col]) {
            continue;
        }
        let top = col;
        let bottom = col + 2u;
        let diagonal = 3u - col;
        let m = material((*cells)[top]);
        if (can_slide((*cells)[top], (*cells)[bottom], (*cells)[diagonal]) && ((r >> (16u + col * 8u)) & 255u) < flow(m)) {
            displace(cells, top, diagonal);
            slid = true;
        }
    }

    if (!fell[0] && !fell[1] && !slid) {
        for (var row = 0u; row < 2u; row += 1u) {
            let left = row * 2u;
            let right = row * 2u + 1u;
            if (((r2 >> (row * 8u)) & 255u) < spread_chance((*cells)[left], (*cells)[right])) {
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
        // all 4 heat results are identical and no random number can matter:
        // no temperature gaps, nothing free to fall, and we exclude what
        // uses its per-cell random number anyway (ignition of wood and oil,
        // fire and smoke, a gas away from 20°C). 4 identical cells never move.
        let v = c00;
        let m = material(v);
        let same = a0 == v && a1 == v && l0 == v && c01 == v && r0 == v && l1 == v
            && c10 == v && c11 == v && r1 == v && b0 == v && b1 == v;
        let fixed = m != WOOD && m != OIL && m != FIRE && m != SMOKE && (kind(m) != GAS || temp(v) == AMBIENT);
        if (same && fixed) {
            let nv = heat(v, v, v, v, v, ux, uy, w, h, frame).cell;
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
    var maybe = array<bool, 4>(false, false, false, false);
    if (wake[0]) { let o0 = heat(c00, l0, c01, a0, c10, ux, uy, w, h, frame); cells[0] = o0.cell; maybe[0] = o0.maybe; }
    if (wake[1]) { let o1 = heat(c01, c00, r0, a1, c11, ux + 1u, uy, w, h, frame); cells[1] = o1.cell; maybe[1] = o1.maybe; }
    if (wake[2]) { let o2 = heat(c10, l1, c11, c00, b0, ux, uy + 1u, w, h, frame); cells[2] = o2.cell; maybe[2] = o2.maybe; }
    if (wake[3]) { let o3 = heat(c11, c10, r1, c01, b1, ux + 1u, uy + 1u, w, h, frame); cells[3] = o3.cell; maybe[3] = o3.maybe; }
    var write = wake;
    for (var k = 0u; k < 4u; k += 1u) {
        if (maybe[k]) {
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
