use sandbox_core::rng::Rng;
use sandbox_core::world::World;
use sandbox_systems::{registry, run_tick};

#[test]
fn headless_600_ticks_is_ten_seconds() {
    let mut w = World::new(64, 48);
    let mut systems = registry();
    let mut rng = Rng::new(7);
    for _ in 0..600 {
        run_tick(&mut w, &mut systems, &mut rng);
    }
    assert!((w.time - 10.0).abs() < 1e-3, "time = {}", w.time);
}
