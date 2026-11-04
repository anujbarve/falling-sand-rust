// Heat, gravity on speed, phase changes (copy of thermal.rs).

// ---------------------------------------------------------------------------
// Heat, gravity on speed, phase changes: one cell. Copy of `heat_cell`.
// ---------------------------------------------------------------------------

fn edge_roll(i: u32, down: u32, key: u32) -> i32 {
    return i32(pcg((2u * i + down) ^ key) & 0xFFFFFFu);
}
// floor((d * rate + roll) / 2^24) without overflow. Copy of flow_24.
// (& on a negative i32 works on its two's complement bits, like Rust.)
fn flow_24(d: i32, rate: i32, roll: i32) -> i32 {
    let hi = d >> 12u;
    let lo = d & 4095;
    return (hi * rate + ((lo * rate + roll) >> 12u)) >> 12u;
}
fn condensed(m: u32) -> bool { return kind(m) != GAS; }
fn radiation_k(ta: i32, tb: i32) -> i32 {
    let kelvin = (ta + tb) / 2 / T_SCALE + 273;
    let t10 = max(kelvin / 10, 0);
    return (((t10 * t10 * t10) >> 6u) * 1045) >> 14u;
}
fn pair_conductance(a: u32, b: u32, ta: i32, tb: i32) -> i32 {
    let ka = cond(a);
    let kb = cond(b);
    let series = 2 * ka * kb / (ka + kb);
    var radiation = 0;
    if (a != b && condensed(a) && condensed(b)) {
        radiation = radiation_k(ta, tb);
    } else if ((a == FIRE && condensed(b)) || (b == FIRE && condensed(a))) {
        radiation = radiation_k(ta, tb) / 3;
    }
    // Water against a hot surface: boundary layer (see pair_conductance in thermal.rs).
    let hot_a = (a == LAVA || a == STONE || a == SAND || a == WOOD) && ta >= celsius(300);
    let hot_b = (b == LAVA || b == STONE || b == SAND || b == WOOD) && tb >= celsius(300);
    if ((a == WATER && hot_b) || (b == WATER && hot_a)) {
        let water_t = select(tb, ta, a == WATER);
        return select(CONVECTIVE_CONDUCTANCE, BOIL_CONDUCTANCE, water_t >= celsius(99));
    }
    return series + radiation;
}
const CONVECTIVE_CONDUCTANCE: i32 = 10000;
const BOIL_CONDUCTANCE: i32 = 250000;
fn heat_rate(m: u32, conductance: i32) -> i32 {
    // Same as heat_rate in thermal.rs.
    var rate: i32;
    if (conductance <= 37000) {
        rate = conductance * RATE_SCALE / cap(m);
    } else {
        rate = (conductance / 8) * RATE_SCALE / max(cap(m) / 8, 1);
    }
    return min(rate, RATE_MAX);
}
fn sky_rate(m: u32, t: i32) -> i32 {
    let t10 = max((t / T_SCALE + 273) / 10, 0);
    let x = (t10 * t10 + 29 * 29) * (t10 + 29);
    return (x / 100) * 1427 / cap(m);
}

fn density(cell: u32) -> i32 {
    let m = material(cell);
    let d = base_density(m);
    let k = kind(m);
    if (k == GAS && m != FIRE) {
        return max(d * 293 * T_SCALE / max(temp(cell) + 273 * T_SCALE, 1), 1);
    }
    if (k == LIQUID) {
        let warmer = (sens(cell) - AMBIENT) / T_SCALE;
        return max(d - d * (expansion(m) * warmer / 1000) / 1000, 1);
    }
    return d;
}

fn sinks(top: u32, bottom: u32) -> bool {
    let kt = kind(material(top));
    let kb = kind(material(bottom));
    // A flame is anchored where it burns: it does not drift off its fuel.
    if (material(top) == FIRE || material(bottom) == FIRE) {
        return false;
    }
    return kb != SOLID && kb != POWDER && density(top) > density(bottom);
}

fn accelerate(cell: u32, heavy: u32, light: u32, medium: u32, roll: u32) -> u32 {
    let dh = density(heavy);
    let dl = density(light);
    var q = 0u;
    if (sinker(material(cell))) {
        q = u32(G_Q * (dh - dl) / dh);
    } else {
        q = u32(G_Q * (dh - dl) / max(dl, 1));
    }
    return min(speed(cell) + ((q + roll) >> 8u), speed_cap(cell, medium));
}

fn speed_cap(cell: u32, medium: u32) -> u32 {
    let m = material(cell);
    let mm = material(medium);
    let own = select(vt_gas(m), vt_liquid(m), kind(mm) == LIQUID);
    return min(own, vt_medium(mm));
}

fn blocked_speed(cell: u32, in_way: u32) -> u32 {
    let mw = material(in_way);
    let same_way = has_speed(mw) && sinker(mw) == sinker(material(cell));
    return min(speed(cell), select(0u, speed(in_way), same_way));
}

fn move_chance(s: u32) -> u32 { return 512u * s / (256u + s); }

struct HeatOut {
    cell: u32,
    maybe: bool, // could a random number have changed it? (chunk sleeping)
}

// Cell (x, y) given its old value and its 4 old neighbours (a neighbour
// outside the grid is ignored, whatever value is passed).
fn heat(me: u32, left: u32, right: u32, up: u32, down: u32, x: u32, y: u32, w: u32, h: u32, frame: u32) -> HeatOut {
    let m = material(me);
    var life = aux(me);
    let t0 = temp(me);
    let t0s = sens(me);
    var maybe = false;

    var t = t0;
    var touches_water = false;
    var touches_air = false;
    var touches_lava = false;
    var touches_fire = false;
    var touches_fuel = false;
    let i = y * w + x;
    let key = pcg(frame ^ EDGE_SALT);
    // Same as heat_cell: an edge with no temperature gap carries no heat
    // and needs no random number. The rate is OURS (our heat capacity).
    var open_faces = 0;
    if (x + 1u < w) {
        let mn = material(right);
        let tn = sens(right);
        touches_water = touches_water || mn == WATER;
        touches_air = touches_air || mn == AIR;
        touches_lava = touches_lava || mn == LAVA;
        touches_fire = touches_fire || mn == FIRE;
        touches_fuel = touches_fuel || mn == WOOD || mn == OIL;
        open_faces += select(0, 1, kind(mn) == GAS && mn != FIRE);
        if (tn != t0s) {
            maybe = true;
            t += flow_24(tn - t0s, heat_rate(m, pair_conductance(m, mn, t0s, tn)), edge_roll(i, 0u, key));
        }
    }
    if (x > 0u) {
        let mn = material(left);
        let tn = sens(left);
        touches_water = touches_water || mn == WATER;
        touches_air = touches_air || mn == AIR;
        touches_lava = touches_lava || mn == LAVA;
        touches_fire = touches_fire || mn == FIRE;
        touches_fuel = touches_fuel || mn == WOOD || mn == OIL;
        open_faces += select(0, 1, kind(mn) == GAS && mn != FIRE);
        if (tn != t0s) {
            maybe = true;
            t -= flow_24(t0s - tn, heat_rate(m, pair_conductance(m, mn, t0s, tn)), edge_roll(i - 1u, 0u, key));
        }
    }
    if (y + 1u < h) {
        let mn = material(down);
        let tn = sens(down);
        touches_water = touches_water || mn == WATER;
        touches_air = touches_air || mn == AIR;
        touches_lava = touches_lava || mn == LAVA;
        touches_fire = touches_fire || mn == FIRE;
        touches_fuel = touches_fuel || mn == WOOD || mn == OIL;
        open_faces += select(0, 1, kind(mn) == GAS && mn != FIRE);
        if (tn != t0s) {
            maybe = true;
            t += flow_24(tn - t0s, heat_rate(m, pair_conductance(m, mn, t0s, tn)), edge_roll(i, 1u, key));
        }
    }
    if (y > 0u) {
        let mn = material(up);
        let tn = sens(up);
        touches_water = touches_water || mn == WATER;
        touches_air = touches_air || mn == AIR;
        touches_lava = touches_lava || mn == LAVA;
        touches_fire = touches_fire || mn == FIRE;
        touches_fuel = touches_fuel || mn == WOOD || mn == OIL;
        open_faces += select(0, 1, kind(mn) == GAS && mn != FIRE);
        if (tn != t0s) {
            maybe = true;
            t -= flow_24(t0s - tn, heat_rate(m, pair_conductance(m, mn, t0s, tn)), edge_roll(i - w, 1u, key));
        }
    }

    let r = pcg(i ^ pcg(frame ^ CELL_SALT));
    let r2 = pcg(r);
    let tick = r2 % TICK_STEPS == 0u;
    // Flame flux onto a touching solid or liquid (same as heat_cell).
    if (touches_fire && condensed(m) && m != FIRE) {
        t += FLAME_UNITS / cap(m);
        maybe = true;
    }
    if (condensed(m) && open_faces > 0 && t0 != AMBIENT) {
        maybe = true;
        t += flow_24(AMBIENT - t0, sky_rate(m, t0) * open_faces, i32(r2 >> 8u));
    }
    if (kind(m) == GAS && m != FIRE) {
        t += ((AMBIENT - t0) * RELAX_K + i32(r & 0xFFFFFu)) >> 20u;
        maybe = maybe || t0 != AMBIENT;
    }

    let has_below = y + 1u < h;
    let has_above = y > 0u;
    if (has_speed(m)) {
        let roll = (r >> 20u) & 255u;
        var free = false;
        var has_other = false;
        var other = 0u;
        if (sinker(m)) {
            has_other = has_below;
            other = down;
            free = has_below && sinks(me, down);
        } else {
            has_other = has_above;
            other = up;
            free = has_above && sinks(up, me);
        }
        if (free) {
            maybe = maybe || speed(me) < speed_cap(me, other);
            if (sinker(m)) {
                life = accelerate(me, me, other, other, roll);
            } else {
                life = accelerate(me, other, me, other, roll);
            }
        } else if (has_other) {
            life = blocked_speed(me, other);
        } else {
            life = 0u;
        }
    }

    var supported = true;
    if (has_below) {
        supported = kind(material(down)) >= POWDER;
    }
    var next = m;
    if (m == WATER) {
        if (t >= celsius(100) + LATENT) {
            next = STEAM;
            life = vt_gas(STEAM);
            t -= LATENT;
        } else if (t <= celsius(-2)) {
            next = ICE;
            t += celsius(10);
        }
    } else if (m == ICE) {
        if (t >= celsius(1)) {
            next = WATER;
            t -= celsius(10);
        }
    } else if (m == STEAM) {
        if (t < celsius(50)) {
            next = WATER;
            t += LATENT;
        }
    } else if (m == LAVA) {
        if (t < celsius(1000)) { next = select(SAND, STONE, supported); }
    } else if (m == STONE) {
        if (t >= celsius(1200)) { next = LAVA; }
    } else if (m == WOOD) {
        if (t >= celsius(300) && (touches_air || touches_lava || touches_fire)) { next = FIRE; life = 120u + (r & 127u); }
    } else if (m == OIL) {
        if (t >= celsius(250) && (touches_air || touches_lava || touches_fire)) { next = FIRE; life = 20u + (r & 31u); }
    } else if (m == FIRE) {
        maybe = true;
        if (touches_water || (!touches_air && !touches_lava && !touches_fuel && !touches_fire) || life == 0u) {
            next = SMOKE;
            life = 30u + (r & 31u);
        } else {
            if (tick && !touches_fuel) { life -= 1u; }
            t = max(t, celsius(900));
        }
    } else if (m == SMOKE) {
        maybe = true;
        if (life == 0u) { next = AIR; } else if (tick) { life -= 1u; }
    }
    if (next != m && next != FIRE && next != SMOKE && !(m == WATER && next == STEAM)) {
        life = 0u;
    }
    let new_cell = pack(next, life, t);
    return HeatOut(new_cell, maybe || new_cell != me);
}

