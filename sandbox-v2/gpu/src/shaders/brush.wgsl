// Brush pass: paints discs into Empty cells, in place on the current buffer.
// Concatenated after common.wgsl. Strokes apply in order; a cell painted by
// an earlier stroke is no longer Empty, so later strokes skip it (the same
// result as painting the strokes one after another).
struct Params {
    w: u32,
    h: u32,
    count: u32,
    empty: u32,
}

struct Stroke {
    cx: i32,
    cy: i32,
    r: i32,
    mat: u32,
    temp: f32,
    fuel: f32,
    pad0: u32,
    pad1: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> strokes: array<Stroke>;
@group(0) @binding(2) var<storage, read_write> cells: array<Cell>;

@compute @workgroup_size(8, 8)
fn stage_brush(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.w || gid.y >= params.h) { return; }
    let i = gid.y * params.w + gid.x;
    var c = cells[i];
    if (c.mat != params.empty) { return; }
    for (var k = 0u; k < params.count; k += 1u) {
        let s = strokes[k];
        let dx = i32(gid.x) - s.cx;
        let dy = i32(gid.y) - s.cy;
        if (dx * dx + dy * dy <= s.r * s.r) {
            c.mat = s.mat;
            c.temp = s.temp;
            c.fuel = s.fuel;
            c.vx = 0.0;
            c.vy = 0.0;
            c.flags = 0u;
            cells[i] = c;
            return;
        }
    }
}
