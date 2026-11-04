// The fused step entry point (one thread per 2x2 block).

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
        // Boiling water (field above 100°C) uses its random number to bubble.
        let fixed = m != WOOD && m != OIL && m != FIRE && m != SMOKE && (kind(m) != GAS || temp(v) == AMBIENT)
            && !(m == WATER && temp(v) > celsius(100));
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

