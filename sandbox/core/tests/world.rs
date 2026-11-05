use sandbox_core::material::Material;
use sandbox_core::world::World;

#[test]
fn new_world_layers_sized() {
    let w = World::new(64, 48);
    assert_eq!(w.cells.width(), 64);
    assert_eq!(w.cells.height(), 48);
    assert_eq!(w.air.vel.len(), 64 * 48);
    assert_eq!(w.air.pressure.len(), 64 * 48);
    assert_eq!(w.air.vapor.len(), 64 * 48);
    assert_eq!(w.waves.len(), 64);
    assert_eq!(w.light.len(), 64 * 48);
    assert!(w.particles.is_empty());
    assert!(w.events.is_empty());
    assert!(w
        .cells
        .as_slice()
        .iter()
        .all(|c| c.mat == Material::Empty && c.temp == 293.0));
}
