//! Per-material physical constants: density, terminal speed and timers.
//!
//! Values are rounded textbook figures at room conditions. Temperatures are
//! kelvin, times are seconds (convert with [`crate::units::ticks`]), speeds are
//! cells per second (1 cell = 1 cm). [`NEVER`] marks "does not apply".
//! `terminal_cps` is used by the granular and liquid passes; `burn_s`,
//! `melt_k`, `boil_k` and `condense_k` by the thermal and fire passes
//! ([`crate::reference_thermal`]). `density` and `evap_s` are not consumed:
//! water boils by latent heat (`reference_thermal::LATENT_WATER`), not by time.

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
    /// Temperature in kelvin below which a gas condenses ([`NEVER`] if it does not).
    pub condense_k: f32,
}

/// Every material id. MUST mirror `sandbox_core::Material` (v1) variant for
/// variant and id for id; `tests/units.rs` checks this. Discriminants index
/// [`MATERIALS`]. Walls are not a material: see [`WALL`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Material {
    /// Nothing; air-like default.
    Empty = 0,
    /// Granular solid that piles up.
    Sand = 1,
    /// Flowing liquid.
    Water = 2,
    /// Flammable liquid that floats on water.
    Oil = 3,
    /// Static solid block.
    Stone = 4,
    /// Heavy natural rock, static.
    Rock = 5,
    /// Flammable static solid.
    Wood = 6,
    /// Light granular burnt residue.
    Ash = 7,
    /// Static solid that conducts heat.
    Metal = 8,
    /// Static transparent solid.
    Glass = 9,
    /// Hot molten liquid.
    Lava = 10,
    /// Corrosive liquid.
    Acid = 11,
    /// Granular solid that explodes when ignited.
    Gunpowder = 12,
    /// Frozen water; melts when warm.
    Ice = 13,
    /// Short-lived burning gas.
    Fire = 14,
    /// Hot rising water vapor.
    Steam = 15,
    /// Rising combustion gas.
    Smoke = 16,
    /// Glowing burning remnant.
    Ember = 17,
}

/// Number of materials.
pub const MATERIAL_COUNT: usize = 18;

/// Out-of-grid / boundary solid id used by the shaders and the reference. It is
/// not a [`Material`] and is never stored in [`MATERIALS`].
pub const WALL: u32 = 18;

const fn d(
    density: f32,
    terminal_cps: f32,
    burn_s: f32,
    evap_s: f32,
    melt_k: f32,
    boil_k: f32,
    condense_k: f32,
) -> MaterialDef {
    MaterialDef {
        density,
        terminal_cps,
        burn_s,
        evap_s,
        melt_k,
        boil_k,
        condense_k,
    }
}

/// Constants for every [`Material`], indexed by `Material as usize`.
///
/// Sources: densities, melting and boiling points from CRC Handbook figures
/// (SiO2 1986 K / 2503 K, glass softening ~1700 K, granite ~1500 K, iron
/// 1811 K / 3134 K, water 273.15 K / 373.15 K, sulfuric-like acid ~610 K boil);
/// terminal speeds are gameplay-scaled from free-fall drag (rain drop ~9 m/s,
/// sand grain ~6 m/s, ash ~3 m/s); burn, evaporation and condensation values
/// are tuning values (wood burns 6 s per the plan). Columns: density, terminal
/// cps, burn_s, evap_s, melt_k, boil_k, condense_k.
pub const MATERIALS: [MaterialDef; MATERIAL_COUNT] = [
    d(1.2, 0.0, NEVER, NEVER, NEVER, NEVER, NEVER), // Empty (air, 20 C)
    d(1600.0, 600.0, NEVER, NEVER, 1986.0, 2503.0, NEVER), // Sand
    d(1000.0, 900.0, NEVER, 2.0, 273.15, 373.15, NEVER), // Water
    d(800.0, 800.0, 8.0, 4.0, 200.0, 520.0, NEVER), // Oil
    d(2700.0, 0.0, NEVER, NEVER, 1500.0, 3000.0, NEVER), // Stone (granite)
    d(3000.0, 0.0, NEVER, NEVER, 1700.0, 3200.0, NEVER), // Rock (basalt)
    d(600.0, 0.0, 6.0, NEVER, NEVER, NEVER, NEVER), // Wood
    d(600.0, 300.0, NEVER, NEVER, NEVER, NEVER, NEVER), // Ash
    d(7870.0, 0.0, NEVER, NEVER, 1811.0, 3134.0, NEVER), // Metal (iron)
    d(2500.0, 0.0, NEVER, NEVER, 1700.0, 2500.0, NEVER), // Glass
    d(2700.0, 300.0, NEVER, NEVER, 1500.0, 3000.0, NEVER), // Lava (molten basalt)
    d(1200.0, 800.0, NEVER, 3.0, 260.0, 610.0, NEVER), // Acid
    d(1000.0, 600.0, 0.5, NEVER, NEVER, NEVER, NEVER), // Gunpowder (bulk)
    d(917.0, 0.0, NEVER, NEVER, 273.15, 373.15, NEVER), // Ice
    d(0.3, 0.0, 1.0, NEVER, NEVER, NEVER, NEVER),   // Fire (flame lives ~1 s)
    d(0.6, 200.0, NEVER, NEVER, NEVER, NEVER, 373.15), // Steam (condenses at 100 C)
    d(1.0, 200.0, NEVER, NEVER, NEVER, NEVER, NEVER), // Smoke
    d(1500.0, 0.0, 3.0, NEVER, NEVER, NEVER, NEVER), // Ember (glowing char)
];

impl Material {
    /// This material's constants.
    pub fn def(self) -> &'static MaterialDef {
        &MATERIALS[self as usize]
    }
}
