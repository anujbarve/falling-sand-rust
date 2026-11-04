//! Part of the physics engine (see mod.rs).

use super::*;

/// Chance (out of 256) that the pair `top` over `bottom` trades places in a
/// step where they may, or 0 if gravity does not move them. The moving
/// "body" is the top cell when it is heavier than air, else the bottom one
/// (a gas rising through air). Bodies with a speed move at that speed;
/// fire, smoke and hot air rise at GAS_RISE times the density gap.
pub(crate) fn pair_chance(top: u32, bottom: u32) -> u32 {
    if !sinks(top, bottom) {
        return 0;
    }
    let body = if sinker(material(top)) { top } else { bottom };
    if has_speed(material(body)) {
        move_chance(speed(body))
    } else {
        let (dt, db) = (density(top), density(bottom));
        move_chance((GAS_RISE * (dt - db) / dt) as u32)
    }
}

/// Could a resting powder or liquid slide from `top` into `diagonal`? Only
/// when it cannot fall straight down and the diagonal cell is lighter.
pub(crate) fn can_slide(top: u32, below: u32, diagonal: u32) -> bool {
    let m = material(top);
    let k = KIND[m as usize];
    (k == POWDER || k == LIQUID) && FLOW[m as usize] > 0 && !sinks(top, below) && sinks(top, diagonal)
}

pub(crate) fn slide_chance(cell: u32) -> u32 {
    let m = material(cell) as usize;
    FLOW[m].max(move_chance(speed(cell)))
}

/// Could steam vent diagonally upward when trapped beneath a heavier cell?
pub(crate) fn can_steam_vent(bottom: u32, top: u32, diagonal: u32) -> bool {
    material(bottom) == STEAM
        && KIND[material(top) as usize] >= LIQUID
        && KIND[material(diagonal) as usize] == GAS
        && sinks(diagonal, bottom)
}

/// Could two side-by-side resting fluid cells trade places (spreading,
/// mixing)? Returns the chance out of 256.
pub(crate) fn spread_chance(left: u32, right: u32) -> u32 {
    let (ml, mr) = (material(left), material(right));
    let fluid = |m: u32| KIND[m as usize] == GAS || KIND[m as usize] == LIQUID;
    // A flame is anchored: it does not mix sideways into air and die out.
    if ml != mr && !anchored(ml) && !anchored(mr) && fluid(ml) && fluid(mr) && speed(left) == 0 && speed(right) == 0 {
        FLOW[ml as usize].min(FLOW[mr as usize])
    } else {
        0
    }
}

/// Swap two cells of a block. A cell that is pushed out of the way stops
/// (its speed was for its own motion, not this one).
pub(crate) fn displace(cells: &mut [u32; 4], mover: usize, pushed: usize) {
    if has_speed(material(cells[pushed])) {
        cells[pushed] = with_aux(cells[pushed], 0);
    }
    cells.swap(mover, pushed);
}

/// Could a nozzle or exhaust in this block act (for some random number, in
/// fact always)? Such a block is never asleep.
pub(crate) fn directed_can_act(cells: &[u32; 4]) -> bool {
    (0..4).any(|c| {
        let m = material(cells[c]);
        (m == JET || m == EXHAUST)
            && block_partner(c, aux(cells[c]) & 3).is_some_and(|p| material(cells[p]) == AIR && p != c)
    })
}

/// Move pass for one 2x2 block. `cells` is in the order top-left,
/// top-right, bottom-left, bottom-right. Speeds and temperatures are packed
/// into the cells, so they travel with them.
/// The cell in the same block as `c` (0 top-left, 1 top-right, 2 bottom-left,
/// 3 bottom-right) that lies in direction `d`, if there is one.
pub(crate) fn block_partner(c: usize, d: u32) -> Option<usize> {
    let (dx, dy) = dir_step(d);
    let (nx, ny) = ((c % 2) as i32 + dx, (c / 2) as i32 + dy);
    ((0..2).contains(&nx) && (0..2).contains(&ny)).then(|| (ny * 2 + nx) as usize)
}

pub fn move_block(cells: &mut [u32; 4], r: u32) {
    let r2 = pcg(r);
    let mut fell = [false; 2];

    // 0. Nozzles and exhaust. A jet puts exhaust into air in its direction;
    //    exhaust moves one cell along its own direction into air. Only pairs
    //    inside this block can act, so a jet works on every other step (its
    //    block only covers its neighbour then), which is deterministic.
    let mut done = [false; 4];
    for c in 0..4 {
        let m = material(cells[c]);
        if (m != JET && m != EXHAUST) || done[c] {
            continue;
        }
        let d = aux(cells[c]) & 3;
        let Some(p) = block_partner(c, d) else { continue };
        if material(cells[p]) != AIR {
            continue;
        }
        if m == JET {
            cells[p] = pack(EXHAUST, d | (EXHAUST_LIFE_MAX << 2), EXHAUST_T);
        } else {
            cells.swap(c, p);
        }
        done[p] = true;
    }

    // 1. Gravity and buoyancy, in each column, at the body's own speed.
    for col in 0..2 {
        let (top, bottom) = (col, col + 2);
        if ((r >> (col * 8)) & 255) < pair_chance(cells[top], cells[bottom]) {
            let (mover, pushed) = if sinker(material(cells[top])) { (top, bottom) } else { (bottom, top) };
            displace(cells, mover, pushed);
            fell[col] = true;
        }
    }

    // 2. Resting powders and liquids that cannot fall straight down slide
    //    diagonally (friction / viscosity set how often). Also steam venting.
    let mut slid = false;
    for col in 0..2 {
        if fell[col] {
            continue;
        }
        let (top, bottom, diagonal) = (col, col + 2, 3 - col);
        if can_slide(cells[top], cells[bottom], cells[diagonal]) && ((r >> (16 + col * 8)) & 255) < slide_chance(cells[top]) {
            let s = speed(cells[top]);
            displace(cells, top, diagonal);
            if s > 0 {
                cells[diagonal] = with_aux(cells[diagonal], s / 2);
            }
            slid = true;
        } else if can_steam_vent(cells[bottom], cells[top], cells[diagonal]) && ((r >> (16 + col * 8)) & 255) < slide_chance(cells[bottom]) {
            displace(cells, bottom, diagonal);
            slid = true;
        }
    }

    // 3. Nothing moved: resting fluids spread sideways. This levels liquids.
    if !fell[0] && !fell[1] && !slid {
        for row in 0..2 {
            let (left, right) = (row * 2, row * 2 + 1);
            if ((r2 >> (row * 8)) & 255) < spread_chance(cells[left], cells[right]) {
                cells.swap(left, right);
            }
        }
    }
}

/// Which corner the block grid starts at in this step: (0,0) or (1,1).
pub fn block_offset(frame: u32) -> usize {
    (frame & 1) as usize
}

