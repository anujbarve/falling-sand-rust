// Air passes. Must match src/air.rs. A header with the tuning consts and
// `is_solid` (generated from air::is_solid) is prepended.
// Bindings are shared by every entry point.
struct Params {
    w: u32,
    h: u32,
    dt: f32,
    wind_x: f32,
    wind_y: f32,
}

@group(0) @binding(0) var<uniform> prm: Params;
@group(0) @binding(1) var<storage, read_write> cells: array<Cell>;
@group(0) @binding(2) var<storage, read_write> vel_a: array<vec2<f32>>;
@group(0) @binding(3) var<storage, read_write> vel_b: array<vec2<f32>>;
@group(0) @binding(4) var<storage, read_write> divg: array<f32>;
@group(0) @binding(5) var<storage, read_write> p_a: array<f32>;
@group(0) @binding(6) var<storage, read_write> p_b: array<f32>;
@group(0) @binding(7) var<storage, read_write> vtmp: array<f32>;

fn dims() -> Dims { return Dims(prm.w, prm.h); }

fn solid_at(i: u32) -> bool { return is_solid(cells[i].mat); }

fn clamp_len(v: vec2<f32>) -> vec2<f32> {
    let len = sqrt(v.x * v.x + v.y * v.y);
    if (len > MAX_AIR_CPS) {
        let s = MAX_AIR_CPS / len;
        return vec2<f32>(v.x * s, v.y * s);
    }
    return v;
}

// Bilinear corners + weights for a backtrace position.
struct Tap { i00: u32, i10: u32, i01: u32, i11: u32, fx: f32, fy: f32 }

fn tap(px_in: f32, py_in: f32) -> Tap {
    let px = min(max(px_in, 0.0), f32(prm.w) - 1.0);
    let py = min(max(py_in, 0.0), f32(prm.h) - 1.0);
    let x0 = i32(floor(px));
    let y0 = i32(floor(py));
    let x1 = min(x0 + 1, i32(prm.w) - 1);
    let y1 = min(y0 + 1, i32(prm.h) - 1);
    let d = dims();
    return Tap(idx(d, x0, y0), idx(d, x1, y0), idx(d, x0, y1), idx(d, x1, y1),
               px - f32(x0), py - f32(y0));
}

fn lerp1(a: f32, b: f32, t: f32) -> f32 { return a * (1.0 - t) + b * t; }
fn lerp2(a: vec2<f32>, b: vec2<f32>, t: f32) -> vec2<f32> {
    return vec2<f32>(a.x * (1.0 - t) + b.x * t, a.y * (1.0 - t) + b.y * t);
}

fn wind() -> vec2<f32> { return clamp_len(vec2<f32>(prm.wind_x, prm.wind_y)); }

fn in_bounds(x: i32, y: i32) -> bool {
    return x >= 0 && y >= 0 && x < i32(prm.w) && y < i32(prm.h);
}

// Pressure of a neighbour; solid or outside -> centre value c.
fn pn(x: i32, y: i32, c: f32) -> f32 {
    if (!in_bounds(x, y)) { return c; }
    let i = idx(dims(), x, y);
    if (solid_at(i)) { return c; }
    return p_a[i];
}

fn cell_id(gid: vec3<u32>) -> vec3<i32> {
    // x, y, valid flag
    let ok = gid.x < prm.w && gid.y < prm.h;
    return vec3<i32>(i32(gid.x), i32(gid.y), select(0, 1, ok));
}

@compute @workgroup_size(8, 8)
fn stage_force(@builtin(global_invocation_id) gid: vec3<u32>) {
    let c = cell_id(gid);
    if (c.z == 0) { return; }
    let i = idx(dims(), c.x, c.y);
    if (solid_at(i)) { vel_a[i] = vec2<f32>(0.0, 0.0); return; }
    var d = cells[i].temp - AMBIENT_K;
    d = select(0.0, d, abs(d) <= MAX_TEMP_EXCESS);
    var v = vel_a[i];
    v.y = v.y - BUOYANCY_K * d * prm.dt;
    let k = max(1.0 - AIR_DRAG * prm.dt, 0.0);
    vel_a[i] = clamp_len(vec2<f32>(v.x * k, v.y * k));
}

@compute @workgroup_size(8, 8)
fn stage_advect_vel(@builtin(global_invocation_id) gid: vec3<u32>) {
    let c = cell_id(gid);
    if (c.z == 0) { return; }
    let i = idx(dims(), c.x, c.y);
    if (solid_at(i)) { vel_b[i] = vec2<f32>(0.0, 0.0); return; }
    let t = clamp_len(vec2<f32>(vel_a[i].x + wind().x, vel_a[i].y + wind().y));
    let q = tap(f32(c.x) - t.x * prm.dt, f32(c.y) - t.y * prm.dt);
    let top = lerp2(vel_a[q.i00], vel_a[q.i10], q.fx);
    let bot = lerp2(vel_a[q.i01], vel_a[q.i11], q.fx);
    vel_b[i] = lerp2(top, bot, q.fy);
}

@compute @workgroup_size(8, 8)
fn stage_div(@builtin(global_invocation_id) gid: vec3<u32>) {
    let c = cell_id(gid);
    if (c.z == 0) { return; }
    let d = dims();
    let i = idx(d, c.x, c.y);
    p_a[i] = 0.0;
    if (solid_at(i)) { divg[i] = 0.0; return; }
    let r = vel_b[idx(d, c.x + 1, c.y)];
    let l = vel_b[idx(d, c.x - 1, c.y)];
    let dn = vel_b[idx(d, c.x, c.y + 1)];
    let u = vel_b[idx(d, c.x, c.y - 1)];
    divg[i] = 0.5 * ((r.x - l.x) + (dn.y - u.y));
}

// Reads binding 5, writes binding 6 (the host swaps the two buffers per sweep).
@compute @workgroup_size(8, 8)
fn stage_jacobi(@builtin(global_invocation_id) gid: vec3<u32>) {
    let c = cell_id(gid);
    if (c.z == 0) { return; }
    let i = idx(dims(), c.x, c.y);
    if (solid_at(i)) { p_b[i] = 0.0; return; }
    let cp = p_a[i];
    let s = pn(c.x + 1, c.y, cp) + pn(c.x - 1, c.y, cp)
          + pn(c.x, c.y - 1, cp) + pn(c.x, c.y + 1, cp);
    p_b[i] = (s - divg[i]) * 0.25;
}

@compute @workgroup_size(8, 8)
fn stage_project(@builtin(global_invocation_id) gid: vec3<u32>) {
    let c = cell_id(gid);
    if (c.z == 0) { return; }
    let i = idx(dims(), c.x, c.y);
    if (solid_at(i)) { vel_a[i] = vec2<f32>(0.0, 0.0); return; }
    let cp = p_a[i];
    let gx = (pn(c.x + 1, c.y, cp) - pn(c.x - 1, c.y, cp)) * 0.5;
    let gy = (pn(c.x, c.y + 1, cp) - pn(c.x, c.y - 1, cp)) * 0.5;
    vel_a[i] = vec2<f32>(vel_b[i].x - gx, vel_b[i].y - gy);
}

fn vapor_at(i: u32) -> f32 {
    if (solid_at(i)) { return 0.0; }
    return cells[i].vapor;
}

@compute @workgroup_size(8, 8)
fn stage_advect_vapor(@builtin(global_invocation_id) gid: vec3<u32>) {
    let c = cell_id(gid);
    if (c.z == 0) { return; }
    let i = idx(dims(), c.x, c.y);
    if (solid_at(i)) { vtmp[i] = 0.0; return; }
    let t = clamp_len(vec2<f32>(vel_a[i].x + wind().x, vel_a[i].y + wind().y));
    let q = tap(f32(c.x) - t.x * prm.dt, f32(c.y) - t.y * prm.dt);
    let top = lerp1(vapor_at(q.i00), vapor_at(q.i10), q.fx);
    let bot = lerp1(vapor_at(q.i01), vapor_at(q.i11), q.fx);
    vtmp[i] = lerp1(top, bot, q.fy);
}

@compute @workgroup_size(8, 8)
fn stage_commit(@builtin(global_invocation_id) gid: vec3<u32>) {
    let c = cell_id(gid);
    if (c.z == 0) { return; }
    let i = idx(dims(), c.x, c.y);
    cells[i].vapor = vtmp[i];
    cells[i].pressure = p_a[i];
}
