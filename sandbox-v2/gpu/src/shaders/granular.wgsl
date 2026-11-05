// Granular (sand) intent: down, else diagonal. Returns the target index or -1.
fn intent(x: i32, y: i32) -> i32 {
    if (y < 0 || (u32(y) & 1u) != params.phase || mat_at(x, y) != SAND) {
        return -1;
    }
    if (is_empty(x, y + 1)) { return lin(x, y + 1); }
    let h = cell_hash(params.tick, u32(x), u32(y), params.seed);
    var d = -1;
    if ((h & 1u) == 1u) { d = 1; }
    if (is_empty(x + d, y + 1)) { return lin(x + d, y + 1); }
    if (is_empty(x - d, y + 1)) { return lin(x - d, y + 1); }
    return -1;
}
