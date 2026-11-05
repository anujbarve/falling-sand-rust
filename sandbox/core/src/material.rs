//! Material identifiers.

/// Kind of matter in a cell. `repr(u8)`; `Empty` is the default air-like state (value 0).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    Empty, Sand, Water, Oil, Stone, Rock, Wood, Ash, Metal, Glass,
    Lava, Acid, Gunpowder, Ice, Fire, Steam, Smoke, Ember,
}
