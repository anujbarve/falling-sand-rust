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
@group(0) @binding(1) var<storage, read> mat_in: array<u32>;
@group(0) @binding(2) var<storage, read> temp_in: array<i32>;
@group(0) @binding(3) var<storage, read_write> mat_out: array<u32>;
@group(0) @binding(4) var<storage, read_write> temp_out: array<i32>;

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

// `var<private>` (not `const`) so they can be indexed with a runtime value.
var<private> KIND: array<u32, 11> = array<u32, 11>(GAS, SOLID, POWDER, LIQUID, LIQUID, LIQUID, GAS, SOLID, GAS, SOLID, GAS);
var<private> DENSITY: array<i32, 11> = array<i32, 11>(10, 255, 150, 100, 80, 250, 3, 255, 9, 255, 6);
var<private> FLOW: array<u32, 11> = array<u32, 11>(256u, 0u, 200u, 230u, 150u, 40u, 200u, 0u, 120u, 0u, 150u);
var<private> COND: array<i32, 11> = array<i32, 11>(15, 100, 40, 120, 40, 80, 20, 40, 80, 150, 15);

fn celsius(c: i32) -> i32 { return c * T_SCALE; }
fn material(cell: u32) -> u32 { return cell & 0xFFu; }
fn aux(cell: u32) -> u32 { return (cell >> 8u) & 0xFFu; }
fn pack(m: u32, a: u32) -> u32 { return m | (a << 8u); }

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

@compute @workgroup_size(16, 16)
fn heat(@builtin(global_invocation_id) id: vec3<u32>) {
    let w = params.width;
    let h = params.height;
    let x = id.x;
    let y = id.y;
    if (x >= w || y >= h) {
        return;
    }
    let i = y * w + x;
    let m = material(mat_in[i]);
    var life = aux(mat_in[i]);
    let t0 = temp_in[i];
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
        let j = js[k];
        let mn = material(mat_in[j]);
        let c = min(k_self, COND[mn]);
        flux += (temp_in[j] - t0) * c / 1024;
        touches_water = touches_water || mn == WATER;
    }
    var t = t0 + flux;
    var supported = true;
    if (y + 1u < h) {
        supported = KIND[material(mat_in[i + w])] >= POWDER;
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
    mat_out[i] = pack(next, life);
    temp_out[i] = t;
}

// ---------------------------------------------------------------------------
// Pass 2: movement. One thread per 2x2 block, in place on the new grid.
// ---------------------------------------------------------------------------

fn density(cell: u32, t: i32) -> i32 {
    let m = material(cell);
    let d = DENSITY[m];
    if (KIND[m] == GAS && m != FIRE) {
        return max(d - (t - AMBIENT) / celsius(40), 1);
    }
    return d;
}

fn sinks(top: u32, t_top: i32, bottom: u32, t_bottom: i32, roll: u32) -> bool {
    let mt = material(top);
    let mb = material(bottom);
    if (KIND[mt] == SOLID || KIND[mb] == SOLID || KIND[mb] == POWDER) {
        return false;
    }
    let dt = density(top, t_top);
    let db = density(bottom, t_bottom);
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

@compute @workgroup_size(16, 16)
fn move_blocks(@builtin(global_invocation_id) id: vec3<u32>) {
    let w = params.width;
    let h = params.height;
    let bx = id.x;
    let by = id.y;
    if (bx >= w / 2u || by >= h / 2u) {
        return;
    }
    let o = params.frame & 1u;
    let x = 2u * bx + o;
    let y = 2u * by + o;
    if (x + 1u >= w || y + 1u >= h) {
        return;
    }
    let idx = array<u32, 4>(y * w + x, y * w + x + 1u, (y + 1u) * w + x, (y + 1u) * w + x + 1u);
    var cells: array<u32, 4>;
    var temps: array<i32, 4>;
    for (var k = 0u; k < 4u; k += 1u) {
        cells[k] = mat_out[idx[k]];
        temps[k] = temp_out[idx[k]];
    }

    let r = hash3(bx, by, params.frame);
    let r2 = pcg(r);
    var fell = array<bool, 2>(false, false);

    for (var col = 0u; col < 2u; col += 1u) {
        let top = col;
        let bottom = col + 2u;
        if (sinks(cells[top], temps[top], cells[bottom], temps[bottom], (r >> (col * 8u)) & 255u)) {
            let c = cells[top]; cells[top] = cells[bottom]; cells[bottom] = c;
            let t = temps[top]; temps[top] = temps[bottom]; temps[bottom] = t;
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
        if (roll < FLOW[m] && sinks(cells[top], temps[top], cells[diagonal], temps[diagonal], 0u)) {
            let c = cells[top]; cells[top] = cells[diagonal]; cells[diagonal] = c;
            let t = temps[top]; temps[top] = temps[diagonal]; temps[diagonal] = t;
            slid = true;
        }
    }

    if (!fell[0] && !fell[1] && !slid) {
        for (var row = 0u; row < 2u; row += 1u) {
            let left = row * 2u;
            let right = row * 2u + 1u;
            if (spreads(cells[left], cells[right], (r2 >> (row * 8u)) & 255u)) {
                let c = cells[left]; cells[left] = cells[right]; cells[right] = c;
                let t = temps[left]; temps[left] = temps[right]; temps[right] = t;
            }
        }
    }

    for (var k = 0u; k < 4u; k += 1u) {
        mat_out[idx[k]] = cells[k];
        temp_out[idx[k]] = temps[k];
    }
}
