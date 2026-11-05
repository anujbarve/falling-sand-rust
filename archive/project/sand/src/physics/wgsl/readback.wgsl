// Packs a rectangle of cells into a linear buffer, row by row, so the CPU
// reads only what it draws (see `step_and_readback_rect` in gpu.rs):
//     out[y * w + x] = cells[(y0 + y) * stride + x0 + x]
// One thread per packed cell, 256 threads per workgroup. The dispatch may be
// 2D (the x dimension is capped at 65535 groups), so the index is rebuilt
// from `num_workgroups`. The bindings are in group 1 so they do not clash
// with the sim's bindings in group 0.

struct PackRect {
    x0: u32,
    y0: u32,
    w: u32,
    h: u32,
    stride: u32, // grid width in cells
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

// Threads per workgroup: must match @workgroup_size of `pack_rect`. Read by gpu.rs (shader_const).
const PACK_WORKGROUP: u32 = 256u;

@group(1) @binding(0) var<uniform> rect: PackRect;
@group(1) @binding(1) var<storage, read> rect_src: array<u32>;
@group(1) @binding(2) var<storage, read_write> rect_out: array<u32>;

@compute @workgroup_size(256)
fn pack_rect(
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(num_workgroups) nwg: vec3<u32>,
) {
    let i = (wid.y * nwg.x + wid.x) * PACK_WORKGROUP + lid.x;
    if i >= rect.w * rect.h {
        return;
    }
    let x = i % rect.w;
    let y = i / rect.w;
    rect_out[i] = rect_src[(rect.y0 + y) * rect.stride + rect.x0 + x];
}
