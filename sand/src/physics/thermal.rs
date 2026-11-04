//! Part of the physics engine (see mod.rs).

use super::*;

// ---------------------------------------------------------------------------
// The rules. cpu.rs calls these; sim.wgsl is a line-by-line copy of them.
// ---------------------------------------------------------------------------

/// Salts that keep the different random numbers of one step independent.
pub(crate) const EDGE_SALT: u32 = 0x5EED_1234;
pub(crate) const CELL_SALT: u32 = 0xA511_E9B3;

/// The random part of rounding the heat flow across one grid edge. An edge
/// is named by the index of its left/upper cell and its direction (0 = to
/// the right, 1 = down), so both cells on the edge use the same number.
/// `key` is `pcg(frame ^ EDGE_SALT)`, computed once per step: one hash per
/// edge is enough here and keeps the GPU fast.
pub(crate) fn edge_roll(i: usize, down: u32, key: u32) -> i32 {
    (pcg((2 * i as u32 + down) ^ key) & 0xFF_FFFF) as i32
}

/// floor((d * rate + roll) / 2^24) without overflowing an i32, for
/// |d| <= 2^20, 0 <= rate <= RATE_MAX and roll < 2^24. (WGSL has no 64-bit
/// integers, so the CPU does it the same way.) Split d = hi * 4096 + lo
/// with 0 <= lo < 4096: then d * rate = (hi * rate) * 4096 + lo * rate, and
/// floor((a * 4096 + b) / 2^24) = floor((a + floor(b / 4096)) / 4096).
pub fn flow_24(d: i32, rate: i32, roll: i32) -> i32 {
    let (hi, lo) = (d >> 12, d & 4095);
    (hi * rate + ((lo * rate + roll) >> 12)) >> 12
}

/// Condensed matter (solid, powder, liquid) radiates and absorbs heat;
/// gases are treated as transparent (except flames).
pub(crate) const fn condensed(m: u32) -> bool {
    KIND[m as usize] != GAS
}

/// Radiative conductance between two touching surfaces, in mW/(m K):
/// 4 * emissivity * sigma * T^3 * CELL, with emissivity 0.9 and T the mean
/// absolute temperature. Negligible at room temperature, larger than
/// rock's conduction above ~1000°C. Computed with T/10 so it fits an i32:
/// (T/10)^3 * 1000 * 1.02e-6 = (T/10)^3 * 1045 / 2^20.
pub fn radiation_k(ta: i32, tb: i32) -> i32 {
    let kelvin = (ta + tb) / 2 / T_SCALE + 273;
    let t10 = (kelvin / 10).max(0);
    (((t10 * t10 * t10) >> 6) * 1045) >> 14
}

/// How much heat crosses the edge between materials `a` and `b` (at
/// temperatures `ta`, `tb`), in mW/(m K): their conductivities in series
/// (the harmonic mean), plus radiation across the boundary between two
/// different materials (inside one continuous body there is no gap to
/// radiate across). A flame radiates onto a surface at about a third of
/// that (emissivity ~0.3).
pub fn pair_conductance(a: u32, b: u32, ta: i32, tb: i32) -> i32 {
    let (ka, kb) = (COND[a as usize], COND[b as usize]);
    let series = 2 * ka * kb / (ka + kb);
    let radiation = if a != b && condensed(a) && condensed(b) {
        radiation_k(ta, tb)
    } else if (a == FIRE && condensed(b)) || (b == FIRE && condensed(a)) {
        radiation_k(ta, tb) / 3
    } else {
        0
    };
    // At this resolution a newly formed hot crust is a whole 5 mm cell.
    // Resolve boiling at the water-facing surface of lava, or of its still
    // incandescent stone/sand crust, using an effective interface transfer.
    // The same conductance is seen from both sides of the edge.
    let hot_quench_face = (a == WATER && matches!(b, LAVA | STONE | SAND) && tb >= celsius(500))
        || (b == WATER && matches!(a, LAVA | STONE | SAND) && ta >= celsius(500));
    if hot_quench_face { (series + radiation) * 6 } else { series + radiation }
}

/// Per-step rate (times 2^24) at which a cell of material `m` follows a
/// temperature gap across an edge of the given conductance: heat crossing
/// the edge divided by the cell's own heat capacity.
pub fn heat_rate(m: u32, conductance: i32) -> i32 {
    (conductance * RATE_SCALE / CAP[m as usize]).min(RATE_MAX)
}

/// Rate (times 2^24, per exposed face) at which a hot surface loses heat by
/// radiating into open air, emissivity 0.9: sigma*eps*(T^2+T0^2)(T+T0) is
/// the exact linearisation of T^4 - T0^4, so loss = rate * (T - T0).
pub(crate) fn sky_rate(m: u32, t: i32) -> i32 {
    let t10 = ((t / T_SCALE + 273) / 10).max(0);
    let x = (t10 * t10 + 29 * 29) * (t10 + 29); // in (10 K)^3
    (x / 100) * 1427 / CAP[m as usize]
}

/// Density of a cell. Gases expand when heated (ideal gas law: density
/// times absolute temperature is constant), so hot air rises: convection.
/// Liquids expand too, much less, so a pot heated from below circulates.
/// Flames keep a fixed density, or they would shoot off what they burn.
pub fn density(cell: u32) -> i32 {
    let (m, t) = (material(cell), temp(cell));
    let d = DENSITY[m as usize];
    let k = KIND[m as usize];
    if k == GAS && m != FIRE {
        // d * 293 K / (t + 273 K), with t in fixed point.
        (d * 293 * T_SCALE / (t + 273 * T_SCALE).max(1)).max(1)
    } else if k == LIQUID {
        // Liquids expand a little: d * (1 - expansion * (t - 20°C)).
        let warmer = (t - AMBIENT) / T_SCALE;
        (d - d * (EXPANSION[m as usize] * warmer / 1000) / 1000).max(1)
    } else {
        d
    }
}

/// Could `top` and the cell right below it trade places by gravity
/// (heavier above lighter)? Solids never move, and a powder is not pushed
/// aside from below.
pub fn sinks(top: u32, bottom: u32) -> bool {
    let mb = material(bottom) as usize;
    // A flame is anchored where it burns: it does not drift off its fuel.
    if material(top) == FIRE || material(bottom) == FIRE {
        return false;
    }
    // A solid can fall into air or liquid (unsupported stone, wood, ice);
    // it still cannot be pushed into another solid or into powder.
    KIND[mb] != SOLID && KIND[mb] != POWDER && density(top) > density(bottom)
}

/// New speed of a falling (or rising) cell after one step of gravity.
/// `heavy` and `light` are the two cells of the pair, `medium` the one it
/// moves through. Buoyancy: the pull is g * (heavy - light) / heavy.
/// The 1/256 part is rounded at random so the average is exact.
pub(crate) fn accelerate(cell: u32, heavy: u32, light: u32, medium: u32, roll: u32) -> u32 {
    let (dh, dl) = (density(heavy), density(light));
    let q = if sinker(material(cell)) {
        (G_Q * (dh - dl) / dh) as u32
    } else {
        (G_Q * (dh - dl) / dl.max(1)) as u32
    };
    (speed(cell) + ((q + roll) >> 8)).min(speed_cap(cell, medium))
}

/// Top speed of `cell` moving through `medium`: its own drag limit in
/// that kind of medium, and the medium's own limit (viscous lava: 0).
pub(crate) fn speed_cap(cell: u32, medium: u32) -> u32 {
    let (m, mm) = (material(cell) as usize, material(medium) as usize);
    let own = if KIND[mm] == LIQUID { VT_LIQUID[m] } else { VT_GAS[m] };
    own.min(VT_MEDIUM[mm])
}

/// The speed the cell may keep when it cannot move: it may not be faster
/// than the thing in its way (a falling column moves together), and it
/// stops dead against anything not moving the same way.
pub(crate) fn blocked_speed(cell: u32, in_way: u32) -> u32 {
    let same_way = has_speed(material(in_way)) && sinker(material(in_way)) == sinker(material(cell));
    speed(cell).min(if same_way { speed(in_way) } else { 0 })
}

/// Chance (out of 256) that a cell with speed level `s` moves in a step
/// where it may. A cell is in the top row of a block only every other
/// step unless it keeps moving, so the chance is p = 2s/(1+s) (s as a
/// fraction): then it covers exactly s cells per step on average.
pub fn move_chance(s: u32) -> u32 {
    512 * s / (256 + s)
}

/// Update one cell for one step: heat conduction from its 4 neighbours,
/// gravity on its speed, then phase changes. Reads only the old grid.
/// Returns the new cell and whether it could have changed for SOME random
/// number (used by chunk sleeping: see below).
pub fn heat_cell(cells: &[u32], w: usize, h: usize, x: usize, y: usize, frame: u32) -> (u32, bool) {
    let i = y * w + x;
    let me = cells[i];
    let m = material(me);
    let mut life = aux(me);
    let t0 = temp(me);
    let mut maybe = false; // could a random number have changed something?

    // 1. Heat across the 4 edges (conduction + radiation), each rounded at
    //    random (see flow_24). The heat crossing an edge is the same for
    //    both cells; each one's temperature changes by that heat over its
    //    own heat capacity. The edge's random number is shared, so between
    //    two cells of the same material heat is conserved exactly (between
    //    different materials, on average). Edges to the right and below are
    //    "ours"; for edges to the left and above we take the neighbour's view
    //    (gap = our temperature minus theirs) and negate it.
    let mut t = t0;
    let mut touches_water = false;
    let mut touches_air = false;
    let mut touches_lava = false;
    let mut touches_fire = false;
    let mut touches_fuel = false; // wood or oil next door: fire can spread into it
    let mut open_faces = 0; // neighbours that are clear gas (sky for radiation)
    let key = pcg(frame ^ EDGE_SALT);
    let mut edge = |j: usize, ours: bool, edge_cell: usize, down: u32| {
        let other = cells[j];
        let (tn, mn) = (temp(other), material(other));
        touches_water |= mn == WATER;
        touches_air |= mn == AIR;
        touches_lava |= mn == LAVA;
        touches_fire |= mn == FIRE;
        touches_fuel |= mn == WOOD || mn == OIL;
        open_faces += (KIND[mn as usize] == GAS && mn != FIRE) as i32;
        if tn == t0 {
            return; // no gap, no flow (and no need for a random number)
        }
        maybe = true;
        let rate = heat_rate(m, pair_conductance(m, mn, t0, tn));
        let roll = edge_roll(edge_cell, down, key);
        t += if ours { flow_24(tn - t0, rate, roll) } else { -flow_24(t0 - tn, rate, roll) };
    };
    if x + 1 < w {
        edge(i + 1, true, i, 0);
    }
    if x > 0 {
        edge(i - 1, false, i - 1, 0);
    }
    if y + 1 < h {
        edge(i + w, true, i, 1);
    }
    if y > 0 {
        edge(i - w, false, i - w, 1);
    }

    let r = pcg(i as u32 ^ pcg(frame ^ CELL_SALT));
    let r2 = pcg(r);
    let tick = r2 % TICK_STEPS == 0; // one tick of fire/smoke life passes
    // A flame touching a solid or liquid heats it with a surface flux of
    // FLAME_Q (150 kW/m^2): dT/dt = q / (rho c dx), in units of 1/256 K per
    // step = 256 q / (CAP*10 * dx * 1200) = FLAME_UNITS / CAP.
    if touches_fire && condensed(m) && m != FIRE {
        t += FLAME_UNITS / CAP[m as usize];
        maybe = true;
    }
    // Hot (or cold) surfaces facing open air radiate to the surroundings.
    // This is how lava really cools: air itself carries little heat.
    if condensed(m) && open_faces > 0 && t0 != AMBIENT {
        maybe = true;
        t += flow_24(AMBIENT - t0, sky_rate(m, t0) * open_faces, (r2 >> 8) as i32);
    }
    // Gases mix with the room and drift back to 20°C.
    if KIND[m as usize] == GAS && m != FIRE {
        t += ((AMBIENT - t0) * RELAX_K + (r & 0xF_FFFF) as i32) >> 20;
        maybe |= t0 != AMBIENT;
    }

    // 2. Gravity. A cell that is free to fall (or, for steam, to rise)
    //    speeds up; one that is blocked slows to what blocks it. The world's
    //    edges block.
    let below = (y + 1 < h).then(|| cells[i + w]);
    let above = (y > 0).then(|| cells[i - w]);
    if has_speed(m) {
        let roll = (r >> 20) & 255;
        let (free, other) = if sinker(m) {
            (below.is_some_and(|b| sinks(me, b)), below)
        } else {
            (above.is_some_and(|a| sinks(a, me)), above)
        };
        life = match (free, other) {
            (true, Some(o)) => {
                let (heavy, light) = if sinker(m) { (me, o) } else { (o, me) };
                // The random rounding of the speed-up can only matter below the cap.
                maybe |= speed(me) < speed_cap(me, o);
                accelerate(me, heavy, light, o, roll)
            }
            (_, Some(o)) => blocked_speed(me, o),
            (_, None) => 0,
        };
    }

    // 3. Phase changes.
    let supported = below.is_none_or(|b| KIND[material(b) as usize] >= POWDER);
    let mut next = m;
    match m {
        WATER if t >= celsius(100) => {
            next = STEAM;
            life = VT_GAS[STEAM as usize];
            t -= celsius(40);
        }
        WATER if t <= celsius(-2) => {
            next = ICE;
            t += celsius(10);
        }
        ICE if t >= celsius(1) => {
            next = WATER;
            t -= celsius(10);
        }
        // Condenses well below boiling, so steam rises a while before raining.
        STEAM if t < celsius(50) => {
            next = WATER;
            t += celsius(20);
        }
        // Lava resting on something sets into stone. Lava that cools while
        // falling or sinking through water shatters into gravel instead
        // (real lava does this in water), so no stone is left hanging in mid-air.
        LAVA if t < celsius(1000) => next = if supported { STONE } else { SAND },
        // Rock melts above eruption temperature, so lava cannot melt it.
        STONE if t >= celsius(1200) => next = LAVA,
        WOOD if t >= celsius(300) && (touches_air || touches_lava || touches_fire) => {
            next = FIRE;
            life = 120 + (r & 127); // 6-13 s of burning
        }
        OIL if t >= celsius(250) && (touches_air || touches_lava || touches_fire) => {
            next = FIRE;
            life = 20 + (r & 31); // 1-2.7 s
        }
        FIRE => {
            maybe = true;
            // Flames go out in water, or with nothing to burn or breathe.
            if touches_water || (!touches_air && !touches_lava && !touches_fuel && !touches_fire) || life == 0 {
                next = SMOKE;
                life = 30 + (r & 31); // 1.6-3.3 s
            } else {
                // Burning releases heat: the flame stays at least this hot.
                // Fuel next door keeps the flame alive while it burns.
                if tick && !touches_fuel {
                    life -= 1;
                }
                t = t.max(celsius(900));
            }
        }
        SMOKE => {
            maybe = true;
            if life == 0 {
                next = AIR;
            } else if tick {
                life -= 1;
            }
        }
        _ => {}
    }
    // A new material starts at rest (fire, smoke, and steam from water got a life above).
    if next != m && next != FIRE && next != SMOKE && !(m == WATER && next == STEAM) {
        life = 0;
    }
    let new = pack(next, life, t);
    (new, maybe || new != me)
}
