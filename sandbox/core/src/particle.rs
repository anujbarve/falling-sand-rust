//! Free-moving particles that live outside the grid.

use crate::material::Material;
use crate::vec2::Vec2;

/// A particle in continuous space. Removed when `life <= 0`.
pub struct Particle {
    /// Position in cell units.
    pub pos: Vec2,
    /// Velocity in cells per second.
    pub vel: Vec2,
    /// Material it will become or represents.
    pub mat: Material,
    /// Temperature, same scale as `Cell::temp`.
    pub temp: f32,
    /// Seconds left; `<= 0` means remove.
    pub life: f32,
}
