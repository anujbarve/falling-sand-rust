//! The `System` trait and the per-tick driver `run_tick`.

use sandbox_core::rng::Rng;
use sandbox_core::time::DT;
use sandbox_core::world::World;

/// One simulation system, stepped once per fixed tick in registry order.
pub trait System {
    /// Stable identifier; a real system keeps the name of the stub it replaces.
    fn name(&self) -> &'static str;
    /// Advances the system by `dt` seconds, mutating the shared `World`.
    fn step(&mut self, w: &mut World, rng: &mut Rng, dt: f32);
}

/// Runs one fixed tick: advances `world.time` by `DT`, clears every cell's
/// `updated` flag, then steps each system in order. Does not drain `world.events`.
pub fn run_tick(world: &mut World, systems: &mut [Box<dyn System>], rng: &mut Rng) {
    world.time += DT;
    for c in world.cells.as_mut_slice() {
        c.updated = false;
    }
    for s in systems.iter_mut() {
        s.step(world, rng, DT);
    }
}
