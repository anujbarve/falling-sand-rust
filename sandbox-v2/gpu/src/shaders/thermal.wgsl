// Thermal pass: sanitize, conduction, moisture cap, phase changes.
// Mirrors reference_thermal::ThermalReference::thermal. src -> dst.
// Debug: number of non-finite temp/fuel values reset (test readback only).
@group(0) @binding(3) var<storage, read_write> sanitized: atomic<u32>;

fn edge(a: u32, b: u32) -> f32 { return bitcast<f32>(EDGE_BITS[tid(a) * TABLE_N + tid(b)]); }

fn san_temp(t: f32) -> f32 {
    if (finite(t)) { return clamp(t, 0.0, MAX_TEMP_K); }
    return AMBIENT_K;
}

@compute @workgroup_size(8, 8)
fn stage_thermal(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.w || gid.y >= params.h) { return; }
    let x = i32(gid.x);
    let y = i32(gid.y);
    let i = lin(x, y);
    var c = src[i];
    let m = c.mat;
    if (!finite(c.temp)) { atomicAdd(&sanitized, 1u); }
    if (!finite(c.fuel)) { atomicAdd(&sanitized, 1u); }
    var t = san_temp(c.temp);
    var f = 0.0;
    if (finite(c.fuel)) { f = clamp(c.fuel, 0.0, MAX_TIMER); }
    var acc = 0.0;
    var wet = false;
    let offs = array<vec2<i32>, 4>(vec2(1, 0), vec2(-1, 0), vec2(0, 1), vec2(0, -1));
    for (var k = 0; k < 4; k += 1) {
        let nx = x + offs[k].x;
        let ny = y + offs[k].y;
        if (!inside(nx, ny)) { continue; }
        let n = src[lin(nx, ny)];
        acc += edge(m, n.mat) * (san_temp(n.temp) - t);
        wet = wet || n.mat == WATER;
    }
    t += acc * bitcast<f32>(INV_C_BITS[tid(m)]);
    if (wet && BURN_TICKS[tid(m)] != 0u) { t = min(t, WATER_BOIL_K); }
    var mat = m;
    if (m == ICE) {
        if (t >= ICE_MELT_K) { mat = WATER; }
    } else if (m == WATER) {
        if (t <= WATER_FREEZE_K) {
            mat = ICE;
        } else if (t >= WATER_BOIL_K) {
            t = WATER_BOIL_K;
            f += 1.0;
            if (f >= EVAP_TICKS) { mat = STEAM; }
        } else {
            f = 0.0;
        }
    } else if (m == STEAM) {
        if (t < STEAM_CONDENSE_K) { mat = WATER; }
    } else if (m == LAVA) {
        if (t < LAVA_FREEZE_K) { mat = ROCK; }
    } else if (m == STONE) {
        if (t >= STONE_MELT_K) { mat = LAVA; }
    } else if (m == ROCK) {
        if (t >= ROCK_MELT_K) { mat = LAVA; }
    } else if (m == SAND) {
        if (t >= SAND_MELT_K) { mat = GLASS; }
    }
    if (mat != m) { f = 0.0; }
    c.mat = mat;
    c.temp = t;
    c.fuel = f;
    dst[i] = c;
}
