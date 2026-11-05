use sandbox_core::vec2::Vec2;

fn v(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

#[test]
fn arithmetic() {
    assert_eq!(v(1.0, 2.0) + v(3.0, 4.0), v(4.0, 6.0));
    assert_eq!(v(3.0, 4.0) - v(1.0, 2.0), v(2.0, 2.0));
    assert_eq!(v(1.0, -2.0) * 3.0, v(3.0, -6.0));
}

#[test]
fn length_and_normalized() {
    assert_eq!(v(3.0, 4.0).length(), 5.0);
    let n = v(3.0, 4.0).normalized();
    assert!((n.x - 0.6).abs() < 1e-6 && (n.y - 0.8).abs() < 1e-6);
    assert_eq!(v(0.0, 0.0).normalized(), v(0.0, 0.0));
}
