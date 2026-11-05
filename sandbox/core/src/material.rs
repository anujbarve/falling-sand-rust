//! Material identifiers.

/// Kind of matter in a cell. `repr(u8)`; `Empty` is the default air-like state (value 0).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    /// Nothing; air-like default.
    Empty,
    /// Granular solid that piles up.
    Sand,
    /// Flowing liquid.
    Water,
    /// Flammable liquid that floats on water.
    Oil,
    /// Static solid block.
    Stone,
    /// Heavy natural rock, static.
    Rock,
    /// Flammable static solid.
    Wood,
    /// Light granular burnt residue.
    Ash,
    /// Static solid that conducts heat.
    Metal,
    /// Static transparent solid.
    Glass,
    /// Hot molten liquid.
    Lava,
    /// Corrosive liquid.
    Acid,
    /// Granular solid that explodes when ignited.
    Gunpowder,
    /// Frozen water; melts when warm.
    Ice,
    /// Short-lived burning gas.
    Fire,
    /// Hot rising water vapor.
    Steam,
    /// Rising combustion gas.
    Smoke,
    /// Glowing burning remnant.
    Ember,
}
