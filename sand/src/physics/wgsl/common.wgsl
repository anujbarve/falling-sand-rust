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
    0u | (4u << 8u) | (4u << 16u) | (4u << 24u),
    4u | (12u << 8u) | (10u << 16u) | (4u << 24u),
    0u | (4u << 8u) | (0u << 16u),
);
const VT_GAS_T = vec3<u32>(
    0u | (255u << 8u) | (255u << 16u) | (255u << 24u),
    255u | (255u << 8u) | (85u << 16u) | (255u << 24u),
    0u | (255u << 8u) | (0u << 16u),
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
const FLAME_UNITS: i32 = 640000; // see FLAME_UNITS in mod.rs

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
    return m == SAND || m == WATER || m == OIL || m == LAVA || m == STEAM || m == STONE || m == WOOD || m == ICE;
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

