// Movement of 2x2 blocks (copy of motion.rs).

// ---------------------------------------------------------------------------
// Movement: one 2x2 block, in registers. Copy of `move_block`.
// ---------------------------------------------------------------------------

fn pair_chance(top: u32, bottom: u32) -> u32 {
    if (!sinks(top, bottom)) {
        return 0u;
    }
    let body = select(bottom, top, sinker(material(top)));
    if (has_speed(material(body))) {
        return move_chance(speed(body));
    }
    let dt = density(top);
    let db = density(bottom);
    return move_chance(u32(GAS_RISE * (dt - db) / dt));
}

fn can_slide(top: u32, below: u32, diagonal: u32) -> bool {
    let m = material(top);
    let k = kind(m);
    return (k == POWDER || k == LIQUID) && flow(m) > 0u && !sinks(top, below) && sinks(top, diagonal);
}

fn slide_chance(cell: u32) -> u32 {
    let m = material(cell);
    return max(flow(m), move_chance(speed(cell)));
}

fn can_steam_vent(bottom: u32, top: u32, diagonal: u32) -> bool {
    return material(bottom) == STEAM
        && kind(material(top)) >= LIQUID
        && kind(material(diagonal)) == GAS
        && sinks(diagonal, bottom);
}

fn is_fluid(k: u32) -> bool {
    return k == GAS || k == LIQUID;
}

fn spread_chance(left: u32, right: u32) -> u32 {
    let ml = material(left);
    let mr = material(right);
    if (ml != mr && is_fluid(kind(ml)) && is_fluid(kind(mr)) && speed(left) == 0u && speed(right) == 0u) {
        return min(flow(ml), flow(mr));
    }
    return 0u;
}

// Could the block change for SOME random number? Copy of `block_can_move`:
// this decides whether the owner chunk counts as active.
fn block_can_move(c: array<u32, 4>) -> bool {
    return pair_chance(c[0], c[2]) > 0u || pair_chance(c[1], c[3]) > 0u
        || can_slide(c[0], c[2], c[3]) || can_slide(c[1], c[3], c[2])
        || can_steam_vent(c[2], c[0], c[3]) || can_steam_vent(c[3], c[1], c[2])
        || spread_chance(c[0], c[1]) > 0u || spread_chance(c[2], c[3]) > 0u;
}

// Cheap first filter: four cells of one material whose density is fixed
// (solids, powders, fire), or four identical cells, can never move
// (block_can_move would say no). Gases and liquids of one material can:
// their density depends on temperature, so hot under cold swaps.
fn may_move(c: array<u32, 4>) -> bool {
    let m = material(c[0]);
    if (material(c[1]) != m || material(c[2]) != m || material(c[3]) != m) {
        return true;
    }
    let k = kind(m);
    if (k == SOLID || k == POWDER || m == FIRE) {
        return false;
    }
    return !(c[1] == c[0] && c[2] == c[0] && c[3] == c[0]);
}

fn displace(cells: ptr<function, array<u32, 4>>, mover: u32, pushed: u32) {
    var p = (*cells)[pushed];
    if (has_speed(material(p))) {
        p = with_aux(p, 0u);
    }
    (*cells)[pushed] = (*cells)[mover];
    (*cells)[mover] = p;
}

fn move_block(cells: ptr<function, array<u32, 4>>, r: u32) {
    let r2 = pcg(r);
    var fell = array<bool, 2>(false, false);

    for (var col = 0u; col < 2u; col += 1u) {
        let top = col;
        let bottom = col + 2u;
        if (((r >> (col * 8u)) & 255u) < pair_chance((*cells)[top], (*cells)[bottom])) {
            if (sinker(material((*cells)[top]))) {
                displace(cells, top, bottom);
            } else {
                displace(cells, bottom, top);
            }
            fell[col] = true;
        }
    }

    var slid = false;
    for (var col = 0u; col < 2u; col += 1u) {
        if (fell[col]) {
            continue;
        }
        let top = col;
        let bottom = col + 2u;
        let diagonal = 3u - col;
        if (can_slide((*cells)[top], (*cells)[bottom], (*cells)[diagonal]) && ((r >> (16u + col * 8u)) & 255u) < slide_chance((*cells)[top])) {
            let s = speed((*cells)[top]);
            displace(cells, top, diagonal);
            if (s > 0u) {
                (*cells)[diagonal] = with_aux((*cells)[diagonal], s / 2u);
            }
            slid = true;
        } else if (can_steam_vent((*cells)[bottom], (*cells)[top], (*cells)[diagonal]) && ((r >> (16u + col * 8u)) & 255u) < slide_chance((*cells)[bottom])) {
            displace(cells, bottom, diagonal);
            slid = true;
        }
    }

    if (!fell[0] && !fell[1] && !slid) {
        for (var row = 0u; row < 2u; row += 1u) {
            let left = row * 2u;
            let right = row * 2u + 1u;
            if (((r2 >> (row * 8u)) & 255u) < spread_chance((*cells)[left], (*cells)[right])) {
                let c = (*cells)[left]; (*cells)[left] = (*cells)[right]; (*cells)[right] = c;
            }
        }
    }
}

