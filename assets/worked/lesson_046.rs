fn blend(left: f32, right: f32, fraction: f32) -> f32 {
    left + (right-left) * fraction.clamp(0.0, 1.0)
}
fn humidity(baseline: f32, delta: f32) -> f32 {
    (baseline + delta).clamp(0.0, 1.0)
}
#[test]
fn weather_blends_and_bounds() {
    assert!((blend(0.2, 0.8, 0.5)-0.5).abs() < 0.0001);
    assert_eq!(blend(0.2, 0.8, -1.0), 0.2);
    assert_eq!(humidity(0.8, 0.5), 1.0);
    assert_eq!(humidity(0.1, -0.5), 0.0);
}
