//! 2D float vector.

use std::ops::{Add, Mul, Sub};

/// A 2D vector of `f32`. Plain value type; `Default` is the zero vector.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    /// Horizontal component.
    pub x: f32,
    /// Vertical component.
    pub y: f32,
}

impl Vec2 {
    /// Euclidean length. Always `>= 0`.
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Unit-length copy; the zero vector maps to the zero vector (never NaN).
    pub fn normalized(self) -> Vec2 {
        let l = self.length();
        if l == 0.0 {
            Vec2::default()
        } else {
            self * (1.0 / l)
        }
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2 { x: self.x + o.x, y: self.y + o.y }
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2 { x: self.x - o.x, y: self.y - o.y }
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, k: f32) -> Vec2 {
        Vec2 { x: self.x * k, y: self.y * k }
    }
}
