#[derive(Clone, Copy, Debug)]
struct Particle { x: f32, y: f32, vx: f32, vy: f32 }
fn integrate(p: &mut Particle, dt: f32, gravity: f32) {
    p.vy += gravity * dt;
    p.x += p.vx * dt;
    p.y += p.vy * dt;
}
#[test]
fn velocity_uses_physics_time_units() {
    let mut p = Particle { x:0.0, y:0.0, vx:20.0, vy:0.0 };
    integrate(&mut p, 0.05, 40.0);
    assert!((p.x-1.0).abs() < 0.0001);
    assert!((p.vy-2.0).abs() < 0.0001);
    assert!((p.y-0.1).abs() < 0.0001);
}
