//! No-op placeholder systems and the fixed-order `registry`.
//! Each later task replaces one stub with a real module, keeping its name string.

use crate::air::AirSystem;
use crate::collisions::Collisions;
use crate::engine::System;
use crate::fire::Fire;
use crate::granular::Granular;
use crate::humidity::Humidity;
use crate::liquid::Liquid;
use crate::particles::Particles;
use crate::thermal::Thermal;
use crate::volcano::Volcano;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;

macro_rules! stub {
    ($ty:ident, $name:literal) => {
        #[doc = concat!("Placeholder for the `", $name, "` system; `step` does nothing.")]
        pub struct $ty;
        impl System for $ty {
            fn name(&self) -> &'static str { $name }
            fn step(&mut self, _w: &mut World, _rng: &mut Rng, _dt: f32) {}
        }
    };
}

stub!(Explosions, "explosions");
stub!(Chemistry, "chemistry");
stub!(Ocean, "ocean");
stub!(Lighting, "lighting");

/// Returns every system in the binding execution order.
pub fn registry() -> Vec<Box<dyn System>> {
    vec![
        Box::new(Granular), Box::new(Liquid), Box::new(AirSystem::default()), Box::new(Thermal),
        Box::new(Humidity), Box::new(Fire::default()), Box::new(Volcano::default()), Box::new(Particles),
        Box::new(Collisions), Box::new(Explosions), Box::new(Chemistry),
        Box::new(Ocean), Box::new(Lighting),
    ]
}
