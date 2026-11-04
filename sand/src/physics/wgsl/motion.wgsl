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
    // A flame is anchored: it does not mix sideways into air and die out.
    if (ml != mr && !anchored(ml) && !anchored(mr) && is_fluid(kind(ml)) && is_fluid(kind(mr)) && speed(left) == 0u && speed(right) == 0u) {
        return min(flow(ml), flow(mr));
    }
    return 0u;
}

// Could the block change for SOME random number? Copy of `block_can_move`:
// this decides whether the owner chunk counts as active.
fn block_can_move(c: array<u32, 4>) -> bool {
    return directed_can_act(c)
        || pair_chance(c[0], c[2]) > 0u || pair_chance(c[1], c[3]) > 0u
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
    if (m == JET || m == EXHAUST) {
        return true; // a nozzle or its exhaust acts every step
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

// Direction codes (aux bits 0-1): 0 up, 1 right, 2 down, 3 left. Copy of dir_step.
fn dir_dx(d: u32) -> i32 { return select(select(0i, 1i, d == 1u), -1i, d == 3u); }
fn dir_dy(d: u32) -> i32 { return select(select(0i, -1i, d == 0u), 1i, d == 2u); }

// The cell in the same block as c in direction d, or 4 if it is outside. Copy of block_partner.
fn block_partner(c: u32, d: u32) -> u32 {
    let nx = i32(c % 2u) + dir_dx(d);
    let ny = i32(c / 2u) + dir_dy(d);
    if (nx < 0i || nx > 1i || ny < 0i || ny > 1i) { return 4u; }
    return u32(ny * 2i + nx);
}

// Copy of directed_can_act.
fn directed_can_act(c: array<u32, 4>) -> bool {
    for (var i = 0u; i < 4u; i += 1u) {
        let m = material(c[i]);
        if (m == JET || m == EXHAUST) {
            let p = block_partner(i, aux(c[i]) & 3u);
            if (p < 4u && material(c[p]) == AIR) { return true; }
        }
    }
    return false;
}

fn move_block(cells: ptr<function, array<u32, 4>>, r: u32) {
    let r2 = pcg(r);
    var fell = array<bool, 2>(false, false);

    // 0. Nozzles and exhaust (copy of the first step of move_block in motion.rs).
    var done = array<bool, 4>(false, false, false, false);
    for (var c = 0u; c < 4u; c += 1u) {
        let m = material((*cells)[c]);
        if ((m == JET || m == EXHAUST) && !done[c]) {
            let d = aux((*cells)[c]) & 3u;
            let p = block_partner(c, d);
            if (p < 4u && material((*cells)[p]) == AIR) {
                if (m == JET) {
                    (*cells)[p] = pack(EXHAUST, d | (EXHAUST_LIFE_MAX << 2u), EXHAUST_T);
                } else {
                    let tmp = (*cells)[c]; (*cells)[c] = (*cells)[p]; (*cells)[p] = tmp;
                }
                done[p] = true;
            }
        }
    }

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

