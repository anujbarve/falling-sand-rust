//! Material colours: a fixed base table plus deterministic per-cell noise.

use sandbox_core::material::Material;

/// Base RGBA colour per material, indexed by `Material as usize` (declaration order).
const BASE: [[u8; 4]; 18] = [
    [0, 0, 0, 0],         // Empty
    [194, 178, 128, 255], // Sand
    [40, 100, 200, 255],  // Water
    [90, 60, 30, 255],    // Oil
    [120, 120, 125, 255], // Stone
    [90, 80, 85, 255],    // Rock
    [110, 75, 40, 255],   // Wood
    [160, 160, 160, 255], // Ash
    [180, 190, 200, 255], // Metal
    [190, 230, 230, 255], // Glass
    [240, 90, 20, 255],   // Lava
    [120, 220, 50, 255],  // Acid
    [60, 55, 50, 255],    // Gunpowder
    [170, 210, 255, 255], // Ice
    [255, 150, 30, 255],  // Fire
    [220, 225, 235, 255], // Steam
    [70, 70, 75, 255],    // Smoke
    [200, 60, 30, 255],   // Ember
];

/// Maximum per-channel deviation applied by [`cell_color`]; must stay at or below 15.
const NOISE_AMPLITUDE: f32 = 12.0;

/// Returns the base RGBA colour of a material. `Empty` has alpha 0.
pub fn base_color(m: Material) -> [u8; 4] {
    BASE[m as usize]
}

/// Hashes `(x, y, seed)` to a value in `-1.0..=1.0`.
fn noise(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1)
        ^ (y as u32).wrapping_mul(0x85EB_CA6B)
        ^ seed.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Returns the colour of a cell: the base colour with a deterministic brightness
/// offset (within 15 per channel) from `(x, y, seed)`. Alpha is unchanged, and
/// `Empty` stays fully transparent.
pub fn cell_color(m: Material, x: i32, y: i32, seed: u32) -> [u8; 4] {
    let base = base_color(m);
    if base[3] == 0 {
        return base;
    }
    let d = (noise(x, y, seed) * NOISE_AMPLITUDE) as i32;
    let adj = |c: u8| (c as i32 + d).clamp(0, 255) as u8;
    [adj(base[0]), adj(base[1]), adj(base[2]), base[3]]
}
