//! Per-material physical constants: density, terminal speed and timers.
//!
//! Values are rounded textbook figures at room conditions. Temperatures are
//! kelvin, times are seconds (convert with [`crate::units::ticks`]), speeds are
//! cells per second (1 cell = 1 cm). [`NEVER`] marks "does not apply". Only
//! `terminal_cps` is consumed so far (granular and liquid passes); the other
//! fields are stored for the thermal and fire passes.

/// "Does not apply" for times and temperatures.
pub const NEVER: f32 = f32::INFINITY;

/// Physical constants of one material.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialDef {
    /// Density in kg/m^3.
    pub density: f32,
    /// Terminal fall speed in cells per second (0 when the material does not fall).
    pub terminal_cps: f32,
    /// Seconds a cell burns before it is consumed ([`NEVER`] if not flammable).
    pub burn_s: f32,
    /// Seconds a cell at boiling point takes to evaporate ([`NEVER`] if it does not).
    pub evap_s: f32,
    /// Melting point in kelvin ([`NEVER`] if it does not melt).
    pub melt_k: f32,
    /// Boiling point in kelvin ([`NEVER`] if it does not boil).
    pub boil_k: f32,
}

/// Every material id. Discriminants index [`MATERIALS`] and equal the
/// `reference::{EMPTY, SAND, WATER, WALL}` ids for the first four.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Material {
    /// Air / nothing.
    Empty = 0,
    /// Dry sand.
    Sand = 1,
    /// Liquid water.
    Water = 2,
    /// Immovable wall.
    Wall = 3,
    /// Rock.
    Stone = 4,
    /// Wood.
    Wood = 5,
    /// Iron-like metal.
    Metal = 6,
    /// Flame.
    Fire = 7,
    /// Molten rock.
    Lava = 8,
    /// Water vapour.
    Steam = 9,
    /// Ash.
    Ash = 10,
}

/// Number of materials.
pub const MATERIAL_COUNT: usize = 11;

/// Constants for every [`Material`], indexed by `Material as usize`.
///
/// Sources: densities, melting and boiling points from CRC Handbook figures
/// (SiO2 1986 K / 2503 K, granite ~1500 K, iron 1811 K / 3134 K, water
/// 273.15 K / 373.15 K); terminal speeds are gameplay-scaled from free-fall
/// drag (rain drop ~9 m/s, sand grain ~6 m/s, ash ~3 m/s); burn and evaporation
/// times are tuning values (wood 6 s per the plan).
pub const MATERIALS: [MaterialDef; MATERIAL_COUNT] = [
    // Empty (air, 20 C)
    MaterialDef { density: 1.2, terminal_cps: 0.0, burn_s: NEVER, evap_s: NEVER, melt_k: NEVER, boil_k: 78.0 },
    // Sand
    MaterialDef { density: 1600.0, terminal_cps: 600.0, burn_s: NEVER, evap_s: NEVER, melt_k: 1986.0, boil_k: 2503.0 },
    // Water
    MaterialDef { density: 1000.0, terminal_cps: 900.0, burn_s: NEVER, evap_s: 2.0, melt_k: 273.15, boil_k: 373.15 },
    // Wall (concrete)
    MaterialDef { density: 2400.0, terminal_cps: 0.0, burn_s: NEVER, evap_s: NEVER, melt_k: NEVER, boil_k: NEVER },
    // Stone (granite)
    MaterialDef { density: 2700.0, terminal_cps: 0.0, burn_s: NEVER, evap_s: NEVER, melt_k: 1500.0, boil_k: 3000.0 },
    // Wood
    MaterialDef { density: 600.0, terminal_cps: 0.0, burn_s: 6.0, evap_s: NEVER, melt_k: NEVER, boil_k: NEVER },
    // Metal (iron)
    MaterialDef { density: 7870.0, terminal_cps: 0.0, burn_s: NEVER, evap_s: NEVER, melt_k: 1811.0, boil_k: 3134.0 },
    // Fire (flame lives about a second)
    MaterialDef { density: 0.3, terminal_cps: 0.0, burn_s: 1.0, evap_s: NEVER, melt_k: NEVER, boil_k: NEVER },
    // Lava (molten basalt)
    MaterialDef { density: 2700.0, terminal_cps: 300.0, burn_s: NEVER, evap_s: NEVER, melt_k: 1500.0, boil_k: 3000.0 },
    // Steam
    MaterialDef { density: 0.6, terminal_cps: 200.0, burn_s: NEVER, evap_s: NEVER, melt_k: NEVER, boil_k: NEVER },
    // Ash
    MaterialDef { density: 600.0, terminal_cps: 300.0, burn_s: NEVER, evap_s: NEVER, melt_k: NEVER, boil_k: NEVER },
];

impl Material {
    /// This material's constants.
    pub fn def(self) -> &'static MaterialDef {
        &MATERIALS[self as usize]
    }
}
