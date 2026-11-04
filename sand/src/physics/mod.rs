//! A second falling-sand engine, built so the same rules can run on the CPU
//! and on the GPU (wgpu compute shaders) and be compared fairly.
//!
//! What is more "physical" than the original engine in `main.rs`:
//!
//! - **Density and buoyancy.** Every material has a density. A heavier fluid
//!   sinks through a lighter one, and the bigger the density gap, the faster
//!   it sinks: sand falls fast through air but slowly through water, oil
//!   rises slowly through water, steam shoots up through air.
//! - **Friction and viscosity.** Powders slide down slopes only some of the
//!   time (friction sets the pile angle). Liquids spread at different speeds:
//!   water fast, oil slower, lava very slowly.
//! - **Heat.** Every cell has a temperature. Heat flows between neighbours
//!   by conduction, at a rate set by how conductive both materials are
//!   (air insulates, ice and water conduct well). Gases also lose heat to the
//!   surroundings.
//! - **Phase changes driven by temperature.** Water boils to steam at 100°C,
//!   steam condenses back into rain when it cools, water freezes, ice melts,
//!   lava cools into stone, very hot stone melts back into lava, and wood and
//!   oil catch fire at their ignition points. Fire burns hot and heats its
//!   surroundings, which is how it spreads.
//!
//! How one step works, and why it parallelises:
//!
//! 1. **Heat pass.** Every cell reads its 4 neighbours from the *old* grid and
//!    writes its new temperature and material into a *new* grid. No cell
//!    writes anywhere another cell writes, so all cells can run at once.
//! 2. **Move pass (Margolus blocks).** The grid is cut into 2x2 blocks. Each
//!    block rearranges only its own 4 cells, so all blocks can run at once.
//!    Every other step the block grid is shifted by one cell diagonally, so
//!    material can cross block borders.
//!
//! The original engine scans cells one by one and lets each move anywhere
//! nearby. That only works in order, so it cannot run on a GPU.
//!
//! All the maths is integer maths and the randomness is a hash of
//! (position, step number), not a running generator. So the CPU and the GPU
//! give exactly the same result, bit for bit, which the tests check.

pub mod clock;
pub mod cpu;
pub mod gpu;

// ---------------------------------------------------------------------------
// Materials
// ---------------------------------------------------------------------------

// A cell is one packed u32, so a step moves half the bytes it would with
// a separate temperature array (the GPU is limited by memory traffic):
//
//   bits 0-3    material (16 possible)
//   bits 4-11   aux: remaining life for fire and smoke
//   bits 12-31  temperature, signed 20 bits (see T_SCALE): about ±2047°C
//
// The GPU shader `sim.wgsl` repeats these numbers: keep the two in sync.
pub const AIR: u32 = 0;
pub const STONE: u32 = 1;
pub const SAND: u32 = 2;
pub const WATER: u32 = 3;
pub const OIL: u32 = 4;
pub const LAVA: u32 = 5;
pub const STEAM: u32 = 6;
pub const WOOD: u32 = 7;
pub const FIRE: u32 = 8;
pub const ICE: u32 = 9;
pub const SMOKE: u32 = 10;
pub const MATERIAL_COUNT: usize = 11;

// What kind of matter each material is.
pub const GAS: u32 = 0;
pub const LIQUID: u32 = 1;
pub const POWDER: u32 = 2;
pub const SOLID: u32 = 3;

pub const NAMES: [&str; MATERIAL_COUNT] = [
    "air", "stone", "sand", "water", "oil", "lava", "steam", "wood", "fire", "ice", "smoke",
];

// ---------------------------------------------------------------------------
// Units. Everything below is in real units, so world time can be scaled:
// the viewer runs as many steps per frame as the chosen speed needs.
// ---------------------------------------------------------------------------

/// One cell is 5 mm across: a 640x360 world is 3.2 m x 1.8 m.
pub const CELL_METERS: f64 = 0.005;
/// One step is 1/1200 s of world time.
pub const STEPS_PER_SECOND: u32 = 1200;
pub const STEP_SECONDS: f64 = 1.0 / STEPS_PER_SECOND as f64;
/// Nothing moves faster than one cell per step: 6 m/s. That is enough to
/// fall the whole 1.8 m window height (5.9 m/s at the bottom).
pub const MAX_SPEED: f64 = CELL_METERS / STEP_SECONDS;
pub const GRAVITY: f64 = 9.81;

/// Speeds are stored as a "level" 0..=255 in the cell's aux byte:
/// level 256 would be MAX_SPEED. `level(1.0)` is 1 m/s.
pub const fn level(meters_per_second: f64) -> u32 {
    (meters_per_second / MAX_SPEED * 256.0) as u32
}
/// Gravity adds this much speed per step, in 1/256 of a level
/// (9.81 m/s^2 * 1/1200 s = 8.2 mm/s per step = 0.349 levels = 89/256).
pub const G_Q: i32 = (GRAVITY * STEP_SECONDS / MAX_SPEED * 65536.0) as i32;

// Indexed by material id.         air stone sand water oil lava steam wood fire ice smoke
pub const KIND: [u32; MATERIAL_COUNT] = [GAS, SOLID, POWDER, LIQUID, LIQUID, LIQUID, GAS, SOLID, GAS, SOLID, GAS];
/// Density in units of 0.1 kg/m^3 (water = 10000). Gases are given at 20°C;
/// `density()` makes hot gases lighter (ideal gas law).
pub const DENSITY: [i32; MATERIAL_COUNT] = [12, 26000, 16000, 10000, 9000, 26000, 8, 6000, 3, 9170, 12];
/// Speed (as a level) at which powders slide down slopes and fluids spread
/// sideways when resting. sand 0.5 m/s, water 1 m/s, oil 0.4, lava 0.05,
/// steam 1, fire 0.5, smoke 0.3. Air's 256 means "never the limit".
pub const FLOW: [u32; MATERIAL_COUNT] = [256, 0, 21, 42, 17, 2, 42, 0, 21, 0, 12];
/// Top speed (level) when sinking or rising through a liquid: viscous drag.
/// About 0.1 m/s for sand, water, oil; 0.3 lava; a steam bubble 0.25 m/s.
pub const VT_LIQUID: [u32; MATERIAL_COUNT] = [0, 0, 4, 4, 4, 12, 10, 0, 0, 0, 0];
/// Top speed (level) anything can reach moving THROUGH this medium. Lava is
/// ~10^5 times more viscous than water: real flow through it is mm/s,
/// below one speed level (2.3 cm/s), so it is 0 (no convection in lava).
pub const VT_MEDIUM: [u32; MATERIAL_COUNT] = [255, 0, 0, 255, 255, 0, 255, 0, 255, 0, 255];
/// Top speed (level) through a gas. No air resistance is modelled, so
/// falling things are only limited by MAX_SPEED; steam is capped at 2 m/s.
pub const VT_GAS: [u32; MATERIAL_COUNT] = [0, 0, 255, 255, 255, 255, 85, 0, 0, 0, 0];
/// Rising speed (level) of gases that do not store a speed (fire, smoke,
/// hot air), at a full density gap: 1 m/s.
pub const GAS_RISE: i32 = 42;
/// Heat, part 1: thermal conductivity in mW/(m K). How easily heat crosses
/// a boundary. Air 0.026 W/(m K), granite 2.5, dry sand 0.3, water 0.6,
/// oil 0.15, lava 1.5, steam 0.025, wood 0.15, flame gas 0.08, ice 2.2.
pub const COND: [i32; MATERIAL_COUNT] = [26, 2500, 300, 600, 150, 1500, 25, 150, 80, 2200, 26];
/// Heat, part 2: volumetric heat capacity rho*c in units of 10 J/(m^3 K).
/// How much heat it takes to warm a cell by 1 K: air 1.2 kJ/(m^3 K),
/// granite 2100, sand 1300, water 4180, oil 1700, lava 3000, wood 1020,
/// flame gas 0.35, ice 1900. Air holds ~3500x less heat than water, so air
/// next to something hot warms up in a fraction of a second.
pub const CAP: [i32; MATERIAL_COUNT] = [120, 210000, 130000, 418000, 170000, 300000, 120, 102000, 35, 190000, 120];
/// Thermal expansion of liquids in ppm per K: hot water is lighter than
/// cold water, which makes liquids heated from below circulate.
pub const EXPANSION: [i32; MATERIAL_COUNT] = [0, 0, 0, 300, 700, 100, 0, 0, 0, 0, 0];
/// Turns conductance (mW/(m K)) / CAP into a per-step rate times 2^24:
/// 2^24 * STEP_SECONDS / CELL^2 / (1000 mW/W * 10 J/unit).
pub const RATE_SCALE: i32 = 55924;
/// Largest per-step rate (times 2^24): 1.6% of a temperature gap per edge.
/// Only flames (almost no heat capacity) reach it; they are kept at 900°C
/// by burning anyway. It also keeps `flow_24` free of overflow.
pub const RATE_MAX: i32 = (1 << 18) - 1;
/// Gases mix with the room and drift back to 20°C with a 3 s time
/// constant: 2^20 / (3 s * 1200 steps/s).
pub const RELAX_K: i32 = 291;

/// Temperatures are fixed point: 256 units per degree Celsius.
pub const T_SCALE: i32 = 256;
pub const fn celsius(c: i32) -> i32 {
    c * T_SCALE
}
pub const AMBIENT: i32 = celsius(20);

/// Temperature range that fits in 20 signed bits.
pub const T_MIN: i32 = -(1 << 19);
pub const T_MAX: i32 = (1 << 19) - 1;

pub const fn material(cell: u32) -> u32 {
    cell & 0xF
}
pub const fn aux(cell: u32) -> u32 {
    (cell >> 4) & 0xFF
}
/// `as i32` reinterprets the bits; `>>` on an i32 copies the sign bit
/// down, so negative temperatures come back out negative.
pub const fn temp(cell: u32) -> i32 {
    (cell as i32) >> 12
}
pub fn pack(material: u32, aux: u32, temp: i32) -> u32 {
    material | (aux << 4) | ((temp.clamp(T_MIN, T_MAX) as u32) << 12)
}

/// Materials that store a speed in their aux byte. (Fire and smoke use
/// aux for their remaining life instead.)
pub const fn has_speed(m: u32) -> bool {
    matches!(m, SAND | WATER | OIL | LAVA | STEAM)
}
/// Heavier than air: falls. Otherwise (steam) it rises.
pub const fn sinker(m: u32) -> bool {
    DENSITY[m as usize] > DENSITY[AIR as usize]
}
/// The cell's speed level (0 for materials without one).
pub const fn speed(cell: u32) -> u32 {
    if has_speed(material(cell)) { aux(cell) } else { 0 }
}
pub const fn with_aux(cell: u32, aux: u32) -> u32 {
    (cell & !(0xFF << 4)) | (aux << 4)
}

/// Fire and smoke lives count "ticks" of 64 steps (53 ms): each step the
/// life goes down by one with chance 1/64.
pub const TICK_STEPS: u32 = 64;

/// Starting temperature of a freshly placed cell.
pub fn default_temp(m: u32) -> i32 {
    match m {
        LAVA => celsius(1150), // basaltic lava erupts at ~1100-1200°C
        FIRE => celsius(900),
        ICE => celsius(-40),
        STEAM => celsius(120),
        _ => AMBIENT,
    }
}

/// Starting `aux` (life in ticks, or speed) of a freshly placed cell.
pub fn default_aux(m: u32) -> u32 {
    match m {
        FIRE => 80,  // 4.3 s
        SMOKE => 60, // 3.2 s
        _ => 0,      // at rest
    }
}

// ---------------------------------------------------------------------------
// Randomness: a hash, not a running generator
// ---------------------------------------------------------------------------

/// PCG hash: scrambles a u32 into a random-looking u32. Pure function, so
/// the GPU (which has no shared random state) computes the same numbers.
pub fn pcg(v: u32) -> u32 {
    let state = v.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277_803_737);
    (word >> 22) ^ word
}

pub fn hash3(a: u32, b: u32, c: u32) -> u32 {
    pcg(a ^ pcg(b ^ pcg(c)))
}

// ---------------------------------------------------------------------------
// The world
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<u32>, // packed cells (material, aux, temperature), row by row, row 0 at the top
    pub frame: u32,      // steps run so far; picks the block offset and the randomness
}

impl World {
    /// An empty world at room temperature with a stone floor.
    pub fn new(width: usize, height: usize) -> Self {
        let mut world = World {
            width,
            height,
            cells: vec![pack(AIR, 0, AMBIENT); width * height],
            frame: 0,
        };
        world.fill(0, height as i32 - 1, width as i32, height as i32, STONE);
        world
    }

    pub fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            None
        } else {
            Some(y as usize * self.width + x as usize)
        }
    }

    pub fn get(&self, x: i32, y: i32) -> u32 {
        self.index(x, y).map_or(STONE, |i| material(self.cells[i]))
    }

    /// Place a fresh cell of material `m` (with its default temperature).
    pub fn set(&mut self, x: i32, y: i32, m: u32) {
        if let Some(i) = self.index(x, y) {
            self.cells[i] = pack(m, default_aux(m), default_temp(m));
        }
    }

    /// Fill the rectangle from (x0, y0) up to, but not including, (x1, y1).
    pub fn fill(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, m: u32) {
        for y in y0..y1 {
            for x in x0..x1 {
                self.set(x, y, m);
            }
        }
    }

    /// Fill a rectangle, but each cell only with chance `percent`/100.
    pub fn sprinkle(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, m: u32, percent: u32) {
        for y in y0..y1 {
            for x in x0..x1 {
                if hash3(x as u32, y as u32, m) % 100 < percent {
                    self.set(x, y, m);
                }
            }
        }
    }

    pub fn count(&self, m: u32) -> usize {
        self.cells.iter().filter(|c| material(**c) == m).count()
    }
}

// ---------------------------------------------------------------------------
// The rules. cpu.rs calls these; sim.wgsl is a line-by-line copy of them.
// ---------------------------------------------------------------------------

/// Salts that keep the different random numbers of one step independent.
const EDGE_SALT: u32 = 0x5EED_1234;
const CELL_SALT: u32 = 0xA511_E9B3;

/// The random part of rounding the heat flow across one grid edge. An edge
/// is named by the index of its left/upper cell and its direction (0 = to
/// the right, 1 = down), so both cells on the edge use the same number.
/// `key` is `pcg(frame ^ EDGE_SALT)`, computed once per step: one hash per
/// edge is enough here and keeps the GPU fast.
fn edge_roll(i: usize, down: u32, key: u32) -> i32 {
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
const fn condensed(m: u32) -> bool {
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
    series + radiation
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
fn sky_rate(m: u32, t: i32) -> i32 {
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
    let (mt, mb) = (material(top) as usize, material(bottom) as usize);
    KIND[mt] != SOLID && KIND[mb] != SOLID && KIND[mb] != POWDER && density(top) > density(bottom)
}

/// New speed of a falling (or rising) cell after one step of gravity.
/// `heavy` and `light` are the two cells of the pair, `medium` the one it
/// moves through. Buoyancy: the pull is g * (heavy - light) / heavy.
/// The 1/256 part is rounded at random so the average is exact.
fn accelerate(cell: u32, heavy: u32, light: u32, medium: u32, roll: u32) -> u32 {
    let (dh, dl) = (density(heavy), density(light));
    let q = (G_Q * (dh - dl) / dh) as u32;
    (speed(cell) + ((q + roll) >> 8)).min(speed_cap(cell, medium))
}

/// Top speed of `cell` moving through `medium`: its own drag limit in
/// that kind of medium, and the medium's own limit (viscous lava: 0).
fn speed_cap(cell: u32, medium: u32) -> u32 {
    let (m, mm) = (material(cell) as usize, material(medium) as usize);
    let own = if KIND[mm] == LIQUID { VT_LIQUID[m] } else { VT_GAS[m] };
    own.min(VT_MEDIUM[mm])
}

/// The speed the cell may keep when it cannot move: it may not be faster
/// than the thing in its way (a falling column moves together), and it
/// stops dead against anything not moving the same way.
fn blocked_speed(cell: u32, in_way: u32) -> u32 {
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
    let mut open_faces = 0; // neighbours that are clear gas (sky for radiation)
    let key = pcg(frame ^ EDGE_SALT);
    let mut edge = |j: usize, ours: bool, edge_cell: usize, down: u32| {
        let other = cells[j];
        let (tn, mn) = (temp(other), material(other));
        touches_water |= mn == WATER;
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
        WATER if t >= celsius(100) => next = STEAM,
        WATER if t <= celsius(-2) => next = ICE,
        ICE if t >= celsius(1) => next = WATER,
        // Condenses well below boiling, so steam rises a while before raining.
        STEAM if t < celsius(50) => next = WATER,
        // Lava resting on something sets into stone. Lava that cools while
        // falling or sinking through water shatters into gravel instead
        // (real lava does this in water), so no stone is left hanging in mid-air.
        LAVA if t < celsius(1000) => next = if supported { STONE } else { SAND },
        // Rock melts above eruption temperature, so lava cannot melt it.
        STONE if t >= celsius(1200) => next = LAVA,
        WOOD if t >= celsius(300) => {
            next = FIRE;
            life = 120 + (r & 127); // 6-13 s of burning
        }
        OIL if t >= celsius(250) => {
            next = FIRE;
            life = 20 + (r & 31); // 1-2.7 s
        }
        FIRE => {
            maybe = true;
            if touches_water || life == 0 {
                next = SMOKE;
                life = 30 + (r & 31); // 1.6-3.3 s
            } else {
                // Burning releases heat: the flame stays at least this hot.
                if tick {
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
    // A new material starts at rest (fire and smoke got a life above).
    if next != m && next != FIRE && next != SMOKE {
        life = 0;
    }
    let new = pack(next, life, t);
    (new, maybe || new != me)
}

/// Chance (out of 256) that the pair `top` over `bottom` trades places in a
/// step where they may, or 0 if gravity does not move them. The moving
/// "body" is the top cell when it is heavier than air, else the bottom one
/// (a gas rising through air). Bodies with a speed move at that speed;
/// fire, smoke and hot air rise at GAS_RISE times the density gap.
fn pair_chance(top: u32, bottom: u32) -> u32 {
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
fn can_slide(top: u32, below: u32, diagonal: u32) -> bool {
    let m = material(top);
    let k = KIND[m as usize];
    (k == POWDER || k == LIQUID) && FLOW[m as usize] > 0 && speed(top) == 0 && !sinks(top, below) && sinks(top, diagonal)
}

/// Could two side-by-side resting fluid cells trade places (spreading,
/// mixing)? Returns the chance out of 256.
fn spread_chance(left: u32, right: u32) -> u32 {
    let (ml, mr) = (material(left), material(right));
    let fluid = |m: u32| KIND[m as usize] == GAS || KIND[m as usize] == LIQUID;
    if ml != mr && fluid(ml) && fluid(mr) && speed(left) == 0 && speed(right) == 0 {
        FLOW[ml as usize].min(FLOW[mr as usize])
    } else {
        0
    }
}

/// Swap two cells of a block. A cell that is pushed out of the way stops
/// (its speed was for its own motion, not this one).
fn displace(cells: &mut [u32; 4], mover: usize, pushed: usize) {
    if has_speed(material(cells[pushed])) {
        cells[pushed] = with_aux(cells[pushed], 0);
    }
    cells.swap(mover, pushed);
}

/// Move pass for one 2x2 block. `cells` is in the order top-left,
/// top-right, bottom-left, bottom-right. Speeds and temperatures are packed
/// into the cells, so they travel with them.
pub fn move_block(cells: &mut [u32; 4], r: u32) {
    let r2 = pcg(r);
    let mut fell = [false; 2];

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
    //    diagonally (friction / viscosity set how often).
    let mut slid = false;
    for col in 0..2 {
        if fell[col] {
            continue;
        }
        let (top, bottom, diagonal) = (col, col + 2, 3 - col);
        let m = material(cells[top]) as usize;
        if can_slide(cells[top], cells[bottom], cells[diagonal]) && ((r >> (16 + col * 8)) & 255) < FLOW[m] {
            displace(cells, top, diagonal);
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

// ---------------------------------------------------------------------------
// Chunk sleeping: skip regions where nothing can happen.
//
// CHOICE: sleeping is a pure optimisation. It never changes the result: a
// chunk is only skipped when every one of its cells provably comes out of the
// step unchanged. A run with sleeping is bit-identical to a run without it
// (tests check this), so the CPU and the GPU agree no matter which chunks each
// one happens to skip.
//
// The grid is cut into fixed CHUNK x CHUNK chunks (independent of the block
// offset). During step n a chunk is ACTIVE if
//   - the heat pass of one of its cells changed it OR could have changed it
//     for SOME random number (`heat_cell`'s second result: any temperature
//     difference to a neighbour, a gas away from 20°C, fire, smoke, or a
//     cell free to fall, all of which use random rounding), or
//   - a 2x2 block it OWNS (the chunk holding the block's top-left cell) could
//     move for SOME random number (`block_can_move`), whether it moved or not.
// Counting "could" and not just "did" matters: a water/oil pair that
// happened not to swap for two steps is not settled, it is unlucky.
//
// Rule: chunk c is AWAKE in step n if c or any of its 8 neighbours was active
// in step n-1 or n-2. Asleep chunks run neither pass (no reads, no writes).
//
// Why skipping an asleep chunk c in step n is exact (by induction on n):
// 1. Every cell within 2 cells of c lies in c's 3x3 chunk neighbourhood N(c)
//    (CHUNK >= 3). Such a cell can only be changed by its own heat update or
//    by a block whose top-left is within 3 cells of c, i.e. owned by a chunk
//    in N(c). So "N(c) inactive in steps n-1 and n-2" means those cells are
//    the same at the start of steps n-2, n-1 and n.
// 2. Heat: a cell's heat update reads it and its 4 neighbours (all within 2
//    of c). An update that could not change the cell for any random number
//    in step n-1 sees the same inputs in step n, so it cannot change it now
//    either. So the post-heat values within 1 cell of c equal those of
//    step n-2.
// 3. Move: step n uses the same block offset as step n-2. Every block that
//    touches c (including blocks owned by an awake neighbour that straddle
//    into c) has its top-left in N(c), sees the same post-heat cells as in
//    step n-2, and could not move then for any random number => cannot now.
// 4. Ping-pong buffers: the heat pass writes into the buffer that holds the
//    state from the start of step n-1. c did not change in step n-1, so that
//    buffer already holds exactly what the skipped heat pass would write.
//    (The CPU swaps two Vecs the same way, so the same argument holds.)
// Waking is always safe, only sleeping needs the argument above. When the
// history is unknown (a fresh upload, a world painted or swapped behind the
// simulator's back) the touched chunks are marked active in the previous
// step, which wakes them and their neighbours for two steps, and both
// buffers get rewritten there before anything sleeps.
//
// The bookkeeping is a per-chunk stamp: the last frame number in which the
// chunk was active. "Active in step n-1 or n-2" is `(n - 1) - stamp < 2`,
// in wrapping u32 maths. (A stale stamp that wraps around after 2^32 steps
// only wakes a chunk for nothing, which is harmless.)
// ---------------------------------------------------------------------------

/// Chunk side, in cells. `sim.wgsl` repeats it; its workgroups map onto
/// chunks (a 16x16 heat workgroup is a quarter chunk; a 16x16 move
/// workgroup of 2x2 blocks is exactly one chunk), so keep it at 32.
pub const CHUNK: usize = 32;

/// Number of chunks across and down for a `w` x `h` grid.
pub fn chunk_dims(w: usize, h: usize) -> (usize, usize) {
    (w.div_ceil(CHUNK), h.div_ceil(CHUNK))
}

/// Could `move_block` change this block for SOME random number `r`?
/// Each candidate swap is checked on the input: if no column swap is
/// possible the later stages see the same cells, so this is exact. When it
/// is false, `move_block` leaves the block as it is.
pub fn block_can_move(cells: &[u32; 4]) -> bool {
    pair_chance(cells[0], cells[2]) > 0
        || pair_chance(cells[1], cells[3]) > 0
        || can_slide(cells[0], cells[2], cells[3])
        || can_slide(cells[1], cells[3], cells[2])
        || spread_chance(cells[0], cells[1]) > 0
        || spread_chance(cells[2], cells[3]) > 0
}

// ---------------------------------------------------------------------------
// Scenes. They are built from fractions of the size, so they work at any
// resolution (the benchmark runs them up to 4K).
// ---------------------------------------------------------------------------

pub const SCENES: [(&str, &str); 5] = [
    ("layers", "sand, water and oil sort themselves by density"),
    ("lava + water", "lava cools to stone, water boils to steam, steam rains back down"),
    ("fire", "fire heats wood until it ignites; a pool of oil goes up too"),
    ("ice", "hot steam and lava melt ice; meltwater refreezes on the cold blocks"),
    ("benchmark", "everything at once; what the benchmark runs"),
];

pub fn scene(index: usize, w: usize, h: usize) -> World {
    let mut g = World::new(w, h);
    let (wi, hi) = (w as i32, h as i32);
    let fx = |f: f32| (f * w as f32) as i32; // fraction of the width, in cells
    let fy = |f: f32| (f * h as f32) as i32;
    let floor = hi - 1;
    match index {
        0 => {
            // A tank holding a jumble of all three; they sort into layers.
            g.fill(fx(0.2), fy(0.3), fx(0.2) + 3, floor, STONE);
            g.fill(fx(0.8), fy(0.3), fx(0.8) + 3, floor, STONE);
            g.sprinkle(fx(0.2) + 3, fy(0.35), fx(0.8), floor, OIL, 30);
            g.sprinkle(fx(0.2) + 3, fy(0.35), fx(0.8), floor, WATER, 35);
            g.sprinkle(fx(0.2) + 3, fy(0.35), fx(0.8), floor, SAND, 25);
        }
        1 => {
            g.fill(fx(0.1), fy(0.55), fx(0.1) + 3, floor, STONE);
            g.fill(fx(0.9), fy(0.55), fx(0.9) + 3, floor, STONE);
            g.fill(fx(0.1) + 3, fy(0.7), fx(0.9), floor, WATER);
            g.fill(fx(0.4), fy(0.05), fx(0.6), fy(0.3), LAVA);
        }
        2 => {
            for i in 0..5 {
                let x = fx(0.08 + i as f32 * 0.12);
                g.fill(x - 2, fy(0.65), x + 2, floor, WOOD); // trunk
                g.fill(x - fx(0.05), fy(0.5), x + fx(0.05), fy(0.65), WOOD); // crown
            }
            g.fill(fx(0.03), floor - 6, fx(0.05), floor, FIRE);
            // An open tub of oil, lit from above.
            g.fill(fx(0.7), fy(0.75), fx(0.7) + 3, floor, STONE);
            g.fill(fx(0.95), fy(0.75), fx(0.95) + 3, floor, STONE);
            g.fill(fx(0.7) + 3, fy(0.8), fx(0.95), floor, OIL);
            g.fill(fx(0.8), fy(0.7), fx(0.82), fy(0.72), FIRE);
        }
        3 => {
            g.fill(fx(0.1), fy(0.6), fx(0.3), floor, ICE);
            g.fill(fx(0.4), fy(0.6), fx(0.6), floor, ICE);
            g.fill(fx(0.7), fy(0.6), fx(0.9), floor, ICE);
            g.fill(fx(0.12), fy(0.1), fx(0.28), fy(0.3), STEAM);
            g.fill(fx(0.45), fy(0.2), fx(0.55), fy(0.3), LAVA);
            g.sprinkle(fx(0.7), fy(0.1), fx(0.9), fy(0.3), WATER, 40);
        }
        _ => {
            // Busy everywhere, so no part of the grid is idle.
            for i in 0..8 {
                let y = fy(0.15 + i as f32 * 0.1);
                let x = if i % 2 == 0 { fx(0.05) } else { fx(0.55) };
                g.fill(x, y, x + fx(0.4), y + 2, STONE); // ledges
            }
            g.sprinkle(0, 0, wi, fy(0.25), SAND, 25);
            g.sprinkle(0, 0, wi, fy(0.25), WATER, 25);
            g.sprinkle(0, fy(0.25), wi, fy(0.5), OIL, 20);
            g.fill(fx(0.45), fy(0.55), fx(0.55), fy(0.65), LAVA);
            g.fill(fx(0.1), fy(0.8), fx(0.2), floor, ICE);
            g.fill(fx(0.8), fy(0.75), fx(0.85), floor, WOOD);
            g.fill(fx(0.82), fy(0.73), fx(0.83), fy(0.74), FIRE);
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpu::CpuSim;

    fn run(world: &mut World, steps: usize) {
        let mut sim = CpuSim::new(world);
        for _ in 0..steps {
            sim.step(world);
        }
    }

    #[test]
    fn single_thread_and_rayon_give_identical_worlds() {
        let mut a = scene(4, 256, 160);
        let mut b = a.clone();
        let mut sim_a = CpuSim::new(&a);
        let mut sim_b = CpuSim::new(&b);
        for _ in 0..200 {
            sim_a.step(&mut a);
            sim_b.step_parallel(&mut b);
        }
        assert_eq!(a, b);
    }

    #[test]
    fn gpu_gives_identical_world_to_cpu() {
        let mut cpu_world = scene(4, 256, 160);
        let mut gpu_world = cpu_world.clone();
        let mut gpu = match gpu::GpuSim::new(&gpu_world) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("no GPU adapter, skipping: {e}");
                return;
            }
        };
        run(&mut cpu_world, 200);
        gpu.step(200);
        gpu.download(&mut gpu_world);
        assert_eq!(cpu_world.frame, gpu_world.frame);
        let diff = (0..cpu_world.cells.len()).filter(|&i| cpu_world.cells[i] != gpu_world.cells[i]).count();
        assert_eq!(diff, 0, "{diff} cells differ between CPU and GPU");
    }

    fn gpu_or_skip(world: &World) -> Option<gpu::GpuSim> {
        match gpu::GpuSim::new(world) {
            Ok(g) => Some(g),
            Err(e) => {
                eprintln!("no GPU adapter, skipping: {e}");
                None
            }
        }
    }

    /// A scene that reaches a true fixed point quickly: still water filling
    /// whole rows of a tank, and a block of sand that falls into a walled
    /// bin and lands flat (a free heap would creep down its slope for many
    /// seconds of world time).
    fn settling_scene(w: usize, h: usize) -> World {
        let mut g = World::new(w, h);
        let (wi, hi) = (w as i32, h as i32);
        let floor = hi - 1;
        g.fill(wi / 2, hi / 2, wi / 2 + 3, floor, STONE);
        g.fill(wi - 3, hi / 2, wi, floor, STONE);
        g.fill(wi / 2 + 3, hi * 3 / 4, wi - 3, floor, WATER);
        g.fill(wi / 8 - 3, hi / 4, wi / 8, floor, STONE);
        g.fill(wi / 3, hi / 4, wi / 3 + 3, floor, STONE);
        g.fill(wi / 8, hi / 4, wi / 3, hi / 3, SAND);
        g
    }

    #[test]
    fn block_can_move_is_never_wrong() {
        // If block_can_move says no, move_block must leave the block alone
        // for every random number.
        let mut seen_stuck = 0;
        for s in 0..20_000u32 {
            let cells: [u32; 4] = std::array::from_fn(|k| {
                let r = hash3(s, k as u32, 77);
                let m = r % MATERIAL_COUNT as u32;
                // Half the cells at rest, the rest with a random speed/life.
                let a = if r & (1 << 31) != 0 { 0 } else { pcg(r) & 255 };
                pack(m, a, AMBIENT + ((r >> 8) % 4096) as i32 * 4 - 8192)
            });
            if block_can_move(&cells) {
                continue;
            }
            seen_stuck += 1;
            for t in 0..64 {
                let mut moved = cells;
                move_block(&mut moved, hash3(s, t, 5));
                assert_eq!(moved, cells, "block said stuck but moved: {cells:x?}");
            }
        }
        assert!(seen_stuck > 1000);
    }

    #[test]
    fn sleeping_gives_identical_world_to_no_sleeping() {
        // Odd sizes on purpose: partial chunks at the right and bottom edges.
        for index in 0..SCENES.len() {
            let mut reference = scene(index, 257, 161);
            let mut world = reference.clone();
            let mut plain = CpuSim::new(&reference);
            let mut sim = CpuSim::new(&world);
            for step in 0..800 {
                if step == 500 {
                    // Paint behind the simulator's back, like the viewer does.
                    for w in [&mut world, &mut reference] {
                        w.fill(20, 10, 30, 20, WATER);
                        w.fill(200, 100, 205, 104, LAVA);
                    }
                }
                plain.step_no_sleep(&mut reference);
                if step % 3 == 0 { sim.step_parallel(&mut world) } else { sim.step(&mut world) }
                if step % 100 == 99 {
                    assert_eq!(world, reference, "scene {index}, step {step}");
                }
            }
        }
    }

    #[test]
    fn gpu_gives_identical_world_to_cpu_at_odd_size() {
        for index in [0, 4] {
            let mut cpu_world = scene(index, 257, 161);
            let mut gpu_world = cpu_world.clone();
            let Some(mut gpu) = gpu_or_skip(&gpu_world) else { return };
            let mut sim = CpuSim::new(&cpu_world);
            for _ in 0..1500 {
                sim.step_no_sleep(&mut cpu_world);
            }
            gpu.step(1500);
            gpu.download(&mut gpu_world);
            let diff = (0..cpu_world.cells.len()).filter(|&i| cpu_world.cells[i] != gpu_world.cells[i]).count();
            assert_eq!(diff, 0, "scene {index}: {diff} cells differ between CPU and GPU");
            assert_eq!(cpu_world.frame, gpu_world.frame);
        }
    }

    #[test]
    fn settled_scene_sleeps_on_cpu_and_gpu() {
        let start = settling_scene(512, 320);
        let mut world = start.clone();
        let mut sim = CpuSim::new(&world);
        let total = sim.chunk_count();
        assert_eq!(total, 16 * 10);
        assert_eq!(sim.awake_chunks(), total, "everything starts awake");
        for _ in 0..9000 {
            sim.step_parallel(&mut world);
        }
        let cpu_awake = sim.awake_chunks();
        assert!(cpu_awake * 10 < total, "only {cpu_awake} of {total} chunks should still be awake");

        // Exactly what an unsleeping run gives.
        let mut reference = start.clone();
        let mut plain = CpuSim::new(&reference);
        for _ in 0..9000 {
            plain.step_no_sleep(&mut reference);
        }
        assert_eq!(world, reference);

        // The GPU agrees, cell for cell and chunk for chunk.
        let Some(mut gpu) = gpu_or_skip(&start) else { return };
        assert_eq!(gpu.chunk_count(), total);
        gpu.step(9000);
        let mut gpu_world = start.clone();
        gpu.download(&mut gpu_world);
        assert_eq!(gpu_world, world);
        assert_eq!(gpu.awake_chunks(), cpu_awake);
        // A fresh upload wakes everything again.
        gpu.upload(&gpu_world);
        assert_eq!(gpu.awake_chunks(), total);
    }

    #[test]
    fn pipelined_readback_and_paint_match_cpu() {
        let mut cpu_world = scene(4, 256, 160);
        let mut shown = cpu_world.clone();
        let mut gpu = match gpu::GpuSim::new(&cpu_world) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("no GPU adapter, skipping: {e}");
                return;
            }
        };
        let mut sim = CpuSim::new(&cpu_world);
        for round in 0..20u32 {
            // Paint the same cells on both sides: a stripe of fire on row 10.
            if round == 7 {
                let edits: Vec<(usize, u32)> =
                    (40..60).map(|x| (10 * 256 + x, pack(FIRE, default_aux(FIRE), default_temp(FIRE)))).collect();
                for &(i, c) in &edits {
                    cpu_world.cells[i] = c;
                }
                gpu.write_cells(&edits);
            }
            for _ in 0..5 {
                sim.step(&mut cpu_world);
            }
            gpu.step_and_readback(5);
            gpu.latest(&mut shown); // may or may not have something new: never blocks
            assert!(shown.frame <= cpu_world.frame);
        }
        gpu.wait();
        assert!(gpu.latest(&mut shown), "the last readback should have arrived");
        assert_eq!(shown.frame, cpu_world.frame);
        assert_eq!(shown, cpu_world);

        // An upload makes readbacks still in flight stale: they must not show.
        gpu.step_and_readback(3);
        let fresh = scene(0, 256, 160);
        gpu.upload(&fresh);
        gpu.wait();
        let before = shown.clone();
        assert!(!gpu.latest(&mut shown), "a readback from before the upload was shown");
        assert_eq!(shown, before);
        gpu.step_and_readback(0);
        gpu.wait();
        assert!(gpu.latest(&mut shown));
        assert_eq!(shown, fresh);

        if gpu.has_timestamps() {
            gpu.step(10);
            gpu.wait();
            let ms = gpu.gpu_time_ms().expect("timestamps should come back after a wait");
            assert!(ms > 0.0 && ms < 1000.0, "implausible GPU time {ms} ms/step");
        }
    }

    /// The GPU works in tiles; sizes that are odd or not a multiple of the
    /// tile size exercise the tile and grid edges, where bugs hide.
    #[test]
    fn gpu_matches_cpu_on_odd_sizes() {
        for (w, h, scene_index) in [(257, 161, 4), (100, 37, 4), (33, 18, 1), (64, 16, 2), (3, 3, 0)] {
            let start = scene(scene_index, w, h);
            let mut gpu = match gpu::GpuSim::new(&start) {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("no GPU adapter, skipping: {e}");
                    return;
                }
            };
            let mut cpu_world = start.clone();
            let mut gpu_world = start.clone();
            let mut sim = CpuSim::new(&cpu_world);
            // Check after an odd and an even number of steps (both offsets).
            for steps in [1, 300] {
                for _ in 0..steps {
                    sim.step(&mut cpu_world);
                }
                gpu.step(steps);
                gpu.download(&mut gpu_world);
                assert_eq!(cpu_world.frame, gpu_world.frame);
                let diff = (0..cpu_world.cells.len()).filter(|&i| cpu_world.cells[i] != gpu_world.cells[i]).count();
                assert_eq!(diff, 0, "{w}x{h}: {diff} cells differ between CPU and GPU at frame {}", cpu_world.frame);
            }
        }
    }

    #[test]
    fn conduction_conserves_heat() {
        // Only stone (no phase change below 1200°C, no gas cooling).
        let mut world = World::new(64, 64);
        world.fill(0, 0, 64, 64, STONE);
        for y in 20..30 {
            for x in 20..30 {
                world.cells[y * 64 + x] = pack(STONE, 0, celsius(1000));
            }
        }
        let total = |w: &World| w.cells.iter().map(|&c| temp(c) as i64).sum::<i64>();
        let before = total(&world);
        run(&mut world, STEPS_PER_SECOND as usize); // 1 s
        let after = total(&world);
        assert_eq!(before, after, "random rounding must still conserve heat exactly");
        // Real granite is slow: in 1 s heat only gets about one cell (5 mm) out.
        let next_to_hot = temp(world.cells[64 * 25 + 19]);
        assert!(next_to_hot > AMBIENT + celsius(5), "the neighbour should warm up (got {next_to_hot})");
        assert_eq!(temp(world.cells[64 * 25 + 5]), AMBIENT, "but not 15 cells away");
    }

    #[test]
    fn oil_floats_on_water() {
        // Start upside down: water on top of oil.
        let mut world = World::new(32, 64);
        world.fill(0, 20, 32, 43, WATER);
        world.fill(0, 43, 32, 63, OIL);
        run(&mut world, 10 * STEPS_PER_SECOND as usize); // 10 s
        let mean_y = |m: u32| {
            let ys: Vec<usize> = (0..world.cells.len()).filter(|&i| material(world.cells[i]) == m).map(|i| i / 32).collect();
            ys.iter().sum::<usize>() as f64 / ys.len() as f64
        };
        assert!(mean_y(OIL) < mean_y(WATER), "oil should end up above water");
    }

    #[test]
    fn lava_heats_water_and_turns_to_stone() {
        // Real 1150°C lava quenches in water: it shatters into gravel or
        // sets into stone within seconds, heating the water around it.
        // (Real sea-entry steam comes from boiling in a sub-millimetre film
        // at the lava surface, which 5 mm cells do not resolve.)
        let mut world = scene(1, 200, 120);
        let mut sim = CpuSim::new(&world);
        let mut hottest_water = AMBIENT;
        for _ in 0..12 * STEPS_PER_SECOND {
            sim.step(&mut world);
            for &c in &world.cells {
                if material(c) == WATER {
                    hottest_water = hottest_water.max(temp(c));
                }
            }
        }
        let hottest = hottest_water / T_SCALE;
        assert!(hottest > 60, "water touching lava should get hot (hottest {hottest}°C)");
        let start = scene(1, 200, 120);
        assert!(world.count(LAVA) < start.count(LAVA), "lava should solidify");
        assert!(world.count(STONE) + world.count(SAND) > start.count(STONE), "into stone or gravel");
    }

    #[test]
    fn lava_ignites_wood() {
        // A painted flame lasts seconds and does not light a plank (true in
        // real life too). Lava sitting on wood mostly radiates into it and
        // gets its surface to 300°C within seconds.
        let mut world = World::new(64, 64);
        world.fill(10, 40, 50, 63, WOOD);
        world.fill(20, 30, 40, 40, LAVA);
        let wood_before = world.count(WOOD);
        run(&mut world, 10 * STEPS_PER_SECOND as usize);
        assert!(world.count(WOOD) < wood_before, "the wood should burn within 10 s");
    }

    #[test]
    fn heat_between_different_materials_is_conserved_on_average() {
        // Hot stone in cold water, no gas anywhere (gases lose heat to the
        // room on purpose). Energy = sum of heat capacity * temperature.
        let mut world = World::new(64, 64);
        world.fill(0, 0, 64, 64, WATER);
        world.fill(24, 24, 40, 40, STONE);
        for y in 24..40 {
            for x in 24..40 {
                world.cells[y * 64 + x] = pack(STONE, 0, celsius(90));
            }
        }
        let energy = |w: &World| {
            w.cells.iter().map(|&c| CAP[material(c) as usize] as f64 * (temp(c) - AMBIENT) as f64).sum::<f64>()
        };
        let before = energy(&world);
        run(&mut world, 5 * STEPS_PER_SECOND as usize);
        let after = energy(&world);
        let drift = (after - before).abs() / before;
        assert!(drift < 0.01, "energy drifted by {:.2}%", drift * 100.0);
        // And it really moved: the water next to the stone warmed up.
        // (Slowly: a 5 mm water cell beside 90°C rock has a time constant of
        // ~100 s, so it gains only a few degrees in 5 s.)
        assert!(temp(world.cells[32 * 64 + 23]) > celsius(21), "water beside the stone should warm");
    }

    #[test]
    fn water_heated_from_below_turns_over() {
        // Hot water under cold water. Conduction alone can only even the
        // two out; with thermal expansion the hot (lighter) water rises, so
        // the top ends up warmer than the bottom.
        let mut world = World::new(32, 66);
        world.fill(0, 1, 32, 65, WATER);
        for y in 33..65 {
            for x in 0..32 {
                world.cells[y * 32 + x] = pack(WATER, 0, celsius(80));
            }
        }
        run(&mut world, 10 * STEPS_PER_SECOND as usize);
        let mean = |rows: std::ops::Range<usize>| {
            let n = rows.len() * 32;
            rows.flat_map(|y| (0..32).map(move |x| y * 32 + x)).map(|i| temp(world.cells[i]) as f64).sum::<f64>()
                / n as f64
                / T_SCALE as f64
        };
        let (top, bottom) = (mean(1..33), mean(33..65));
        assert!(top > bottom + 10.0, "top {top:.1}°C should be warmer than bottom {bottom:.1}°C");
    }

    #[test]
    fn lava_in_open_air_crusts_over() {
        // Radiation to the surroundings cools exposed lava; air alone would
        // take far longer. Lava resting on the floor sets into stone.
        let mut world = World::new(64, 64);
        world.fill(16, 54, 48, 63, LAVA);
        let stone_before = world.count(STONE);
        run(&mut world, 30 * STEPS_PER_SECOND as usize);
        assert!(world.count(STONE) > stone_before + 20, "a crust should form within 30 s");
    }

    /// Steps until a grain dropped from rest reaches the floor, averaged
    /// over several grains (the speed-up is random rounding).
    fn mean_landing_step(world: &mut World, xs: &[usize], landing_y: usize, max_steps: usize) -> f64 {
        let w = world.width;
        let mut sim = CpuSim::new(world);
        let mut landed = vec![None; xs.len()];
        for step in 1..=max_steps {
            sim.step(world);
            for (k, &x) in xs.iter().enumerate() {
                if landed[k].is_none() && material(world.cells[landing_y * w + x]) == SAND {
                    landed[k] = Some(step);
                }
            }
            if landed.iter().all(|l| l.is_some()) {
                break;
            }
        }
        let steps: Vec<usize> = landed.iter().map(|l| l.expect("every grain should land")).collect();
        steps.iter().sum::<usize>() as f64 / steps.len() as f64
    }

    #[test]
    fn free_fall_takes_the_right_time() {
        // Drop grains 238 cells (1.19 m) through air: t = sqrt(2h/g) = 0.493 s.
        let mut world = World::new(64, 260);
        let xs: Vec<usize> = (4..64).step_by(8).collect();
        for &x in &xs {
            world.set(x as i32, 20, SAND);
        }
        let h = (258 - 20) as f64 * CELL_METERS;
        let expected = (2.0 * h / GRAVITY).sqrt() * STEPS_PER_SECOND as f64;
        let got = mean_landing_step(&mut world, &xs, 258, 3000);
        let err = (got - expected).abs() / expected;
        assert!(err < 0.1, "fell in {got:.0} steps, expected {expected:.0} ({:.1}% off)", err * 100.0);
    }

    #[test]
    fn sand_sinks_slowly_through_water() {
        // Through 0.6 m of water at its ~0.1 m/s terminal speed: about 6.4 s,
        // where the same drop through air takes 0.35 s.
        let mut world = World::new(32, 142);
        world.fill(0, 20, 32, 141, WATER);
        let xs = [8, 16, 24];
        for &x in &xs {
            world.set(x as i32, 19, SAND);
        }
        let got = mean_landing_step(&mut world, &xs, 140, 20 * STEPS_PER_SECOND as usize) / STEPS_PER_SECOND as f64;
        assert!((4.5..9.0).contains(&got), "sank in {got:.2} s");
    }
}
