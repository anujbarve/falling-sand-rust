// Shared cell layout. Must match `CellRaw` in src/layout.rs (32 bytes).
struct Cell {
    mat: u32,
    temp: f32,
    fuel: f32,
    vx: f32,
    vy: f32,
    vapor: f32,
    pressure: f32,
    flags: u32,
}

struct Dims {
    w: u32,
    h: u32,
}

// Bounds-checked cell index. Out-of-range coordinates clamp to the edge so a
// buffer access is never out of bounds. Callers that need "outside" semantics
// must test x/y themselves first.
fn idx(dims: Dims, x: i32, y: i32) -> u32 {
    let cx = u32(clamp(x, 0, i32(dims.w) - 1));
    let cy = u32(clamp(y, 0, i32(dims.h) - 1));
    return cy * dims.w + cx;
}
