//! Part of the physics engine (see mod.rs).

use super::*;


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
pub const VT_LIQUID: [u32; MATERIAL_COUNT] = [0, 4, 4, 4, 4, 12, 10, 4, 0, 4, 0];
/// Top speed (level) anything can reach moving THROUGH this medium. Lava is
/// ~10^5 times more viscous than water: real flow through it is mm/s,
/// below one speed level (2.3 cm/s), so it is 0 (no convection in lava).
pub const VT_MEDIUM: [u32; MATERIAL_COUNT] = [255, 0, 0, 255, 255, 0, 255, 0, 255, 0, 255];
/// Top speed (level) through a gas. No air resistance is modelled, so
/// falling things are only limited by MAX_SPEED; steam is capped at 2 m/s.
pub const VT_GAS: [u32; MATERIAL_COUNT] = [0, 255, 255, 255, 255, 255, 85, 255, 0, 255, 0];
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
/// Flame heat flux onto a touching surface (W/m^2), and the per-step heating
/// it gives in 1/256 K units, numerator: 256 * 150000 / (10 * 0.005 * 1200).
pub const FLAME_UNITS: i32 = 640_000;
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

/// Latent heat of vaporisation of water, as the temperature rise it is worth
/// in water: L / c = 2.26 MJ/kg / 4.18 kJ/(kg K) = 541 K. Boiling water stores
/// this in its temperature field above 100°C until it has all turned to steam.
pub const LATENT_K: i32 = 541;
pub const LATENT: i32 = LATENT_K * T_SCALE;

/// The temperature a cell counts as for conduction and density. Water above
/// 100°C is still boiling: its field holds the stored latent heat, so it
/// counts as 100°C here.
pub fn sens(cell: u32) -> i32 {
    let t = temp(cell);
    if material(cell) == WATER && t > celsius(100) { celsius(100) } else { t }
}
pub fn pack(material: u32, aux: u32, temp: i32) -> u32 {
    material | (aux << 4) | ((temp.clamp(T_MIN, T_MAX) as u32) << 12)
}

/// Materials that store a speed in their aux byte. (Fire and smoke use
/// aux for their remaining life instead.)
pub const fn has_speed(m: u32) -> bool {
    matches!(m, SAND | WATER | OIL | LAVA | STEAM | STONE | WOOD | ICE)
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

