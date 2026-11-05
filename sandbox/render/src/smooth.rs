//! Motion smoothing: blend the previous and current simulation snapshots for drawing.
//!
//! The app computes `alpha = accumulator / DT` from `FixedStep` and draws the blend.

/// Linear interpolation: `a` at `t = 0`, `b` at `t = 1`.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Writes `lerp(prev[i], cur[i], alpha)` into `out[i]` for every index of the shortest slice.
pub fn interp_world_alpha(prev: &[f32], cur: &[f32], alpha: f32, out: &mut [f32]) {
    for ((o, p), c) in out.iter_mut().zip(prev).zip(cur) {
        *o = lerp(*p, *c, alpha);
    }
}
