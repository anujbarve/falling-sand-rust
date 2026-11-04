// Chunk wake pass.

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

