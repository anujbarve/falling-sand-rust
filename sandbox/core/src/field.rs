//! Scalar field and air layer storage.

use crate::vec2::Vec2;

/// A scalar field. Invariant: `data.len() == w * h`.
pub struct Field {
    /// Width in cells.
    pub w: usize,
    /// Height in cells.
    pub h: usize,
    /// Row-major values.
    pub data: Vec<f32>,
}

/// Air layer. Invariant: every vector has length `w * h`, row-major.
pub struct Air {
    /// Width in cells.
    pub w: usize,
    /// Height in cells.
    pub h: usize,
    /// Velocity per cell.
    pub vel: Vec<Vec2>,
    /// Pressure per cell.
    pub pressure: Vec<f32>,
    /// Humidity per cell, 0..1.
    pub vapor: Vec<f32>,
}
