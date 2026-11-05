// Particle step. Rule defined by src/reference_particles.rs; must match it
// bit for bit. Concatenated after the header generated in particles.rs and
// after common.wgsl (for `Cell`).
//
// Bindings: 0 uniform params, 1 particles (rw), 2 cells (read), 3 counter (rw).

struct Particle {
    pos: vec2<f32>,
    vel: vec2<f32>,
    mat: u32,
    temp: f32,
    life: f32,
    pad: f32,
}

struct Params {
    count: u32,
    w: u32,
    h: u32,
    hz: i32,
    grav_fp: i32,
    dt: f32,
    fric_fp: i32,
    pad1: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> parts: array<Particle>;
@group(0) @binding(2) var<storage, read> cells: array<Cell>;
@group(0) @binding(3) var<storage, read_write> sanitized: atomic<u32>;

// Bit-pattern tests (not x != x, which fast-math may fold).
fn finite(v: f32) -> bool { return (bitcast<u32>(v) & 0x7f800000u) != 0x7f800000u; }
fn is_nan(v: f32) -> bool { return !finite(v) && (bitcast<u32>(v) & 0x007fffffu) != 0u; }

fn to_fp(x: f32) -> i32 { return i32(floor(x * f32(FP_ONE))); }
fn from_fp(i: i32) -> f32 { return f32(i) * (1.0 / f32(FP_ONE)); }
fn vel_fp(v: f32) -> i32 { return to_fp(clamp(v, -MAX_SPEED_CPS, MAX_SPEED_CPS)); }

fn mat_at(x: i32, y: i32) -> u32 {
    if x < 0 || y < 0 || x >= i32(params.w) || y >= i32(params.h) {
        return WALL;
    }
    return cells[u32(y) * params.w + u32(x)].mat;
}

@compute @workgroup_size(256)
fn step(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= params.count {
        return;
    }
    var p = parts[i];
    let ok = finite(p.pos.x) && finite(p.pos.y) && finite(p.vel.x) && finite(p.vel.y)
        && finite(p.temp) && !is_nan(p.life);
    if !ok {
        parts[i] = Particle(vec2<f32>(0.0), vec2<f32>(0.0), p.mat, 0.0, 0.0, 0.0);
        atomicAdd(&sanitized, 1u);
        return;
    }
    if p.life <= 0.0 {
        return;
    }
    let inside = p.pos.x >= 0.0 && p.pos.y >= 0.0
        && p.pos.x < f32(params.w) && p.pos.y < f32(params.h);
    var px = -1;
    var py = -1;
    if inside {
        px = to_fp(p.pos.x);
        py = to_fp(p.pos.y);
    }
    var cx = px >> FP_BITS;
    var cy = py >> FP_BITS;
    if !inside || is_solid(mat_at(cx, cy)) {
        parts[i].life = 0.0;
        return;
    }
    var vx = vel_fp(p.vel.x);
    let vy0 = vel_fp(p.vel.y);
    let supported = (py & (FP_ONE - 1)) == FP_ONE - 1
        && is_solid(mat_at(cx, cy + 1))
        && abs(vy0) <= REST_VY_FP;
    var vy = 0;
    if supported {
        if vx > 0 { vx = max(vx - params.fric_fp, 0); } else { vx = min(vx + params.fric_fp, 0); }
    } else {
        vy = clamp(vy0 + params.grav_fp, -MAX_SPEED_FP, MAX_SPEED_FP);
    }
    let dx = vx / params.hz;
    let dy = vy / params.hz;
    let ex = (px + dx) >> FP_BITS;
    let ey = (py + dy) >> FP_BITS;
    let ax = abs(dx);
    let ay = abs(dy);
    let sx = sign(dx);
    let sy = sign(dy);
    var hit = 0;
    var distx = 0;
    var disty = 0;
    loop {
        let needx = cx != ex;
        let needy = cy != ey;
        if !needx && !needy {
            break;
        }
        if sx > 0 { distx = ((cx + 1) << FP_BITS) - px; } else { distx = px - (cx << FP_BITS) + 1; }
        if sy > 0 { disty = ((cy + 1) << FP_BITS) - py; } else { disty = py - (cy << FP_BITS) + 1; }
        var stepx = needx;
        if needx && needy {
            stepx = distx * ay <= disty * ax;
        }
        var nx = cx;
        var ny = cy;
        if stepx { nx = cx + sx; } else { ny = cy + sy; }
        if is_solid(mat_at(nx, ny)) {
            if stepx { hit = 1; } else { hit = 2; }
            break;
        }
        cx = nx;
        cy = ny;
    }
    let lo_x = cx << FP_BITS;
    let lo_y = cy << FP_BITS;
    let top = FP_ONE - 1;
    if hit == 1 {
        py = clamp(py + dy * distx / ax, lo_y, lo_y + top);
        if sx > 0 { px = lo_x + top; } else { px = lo_x; }
        vx = -(vx * REST_NUM) / REST_DEN;
    } else if hit == 2 {
        px = clamp(px + dx * disty / ay, lo_x, lo_x + top);
        if sy > 0 { py = lo_y + top; } else { py = lo_y; }
        vy = -(vy * REST_NUM) / REST_DEN;
    } else {
        px += dx;
        py += dy;
    }
    p.pos = vec2<f32>(from_fp(px), from_fp(py));
    p.vel = vec2<f32>(from_fp(vx), from_fp(vy));
    p.life = p.life - params.dt;
    if p.life <= 0.0 {
        p.life = 0.0;
    }
    parts[i] = p;
}
