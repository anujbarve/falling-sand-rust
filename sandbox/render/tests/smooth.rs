//! Tests for render-side motion smoothing.

use sandbox_render::smooth::{interp_world_alpha, lerp};

#[test]
fn lerp_endpoints() {
    assert_eq!(lerp(2.0, 8.0, 0.0), 2.0);
    assert_eq!(lerp(2.0, 8.0, 1.0), 8.0);
    assert_eq!(lerp(-3.0, 5.0, 0.5), 1.0);
}

#[test]
fn interp_midpoint() {
    let prev: [f32; 3] = [0.0, 1.0, 10.0];
    let cur = [2.0, 1.0, -10.0];
    let mut out = [9.0; 3];
    interp_world_alpha(&prev, &cur, 0.5, &mut out);
    for i in 0..3 {
        assert!((out[i] - (prev[i] + cur[i]) / 2.0).abs() < 1e-6);
    }
}

#[test]
fn interp_alpha_extremes() {
    let prev = [1.0, 2.0];
    let cur = [3.0, 4.0];
    let mut out = [0.0; 2];
    interp_world_alpha(&prev, &cur, 0.0, &mut out);
    assert_eq!(out, prev);
    interp_world_alpha(&prev, &cur, 1.0, &mut out);
    assert_eq!(out, cur);
}
