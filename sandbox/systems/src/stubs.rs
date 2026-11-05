//! The fixed-order system `registry`. No placeholder systems remain.

use crate::air::AirSystem;
use crate::collisions::Collisions;
use crate::engine::System;
use crate::chemistry::Chemistry;
use crate::explosions::Explosions;
use crate::fire::Fire;
use crate::granular::Granular;
use crate::humidity::Humidity;
use crate::lighting::Lighting;
use crate::liquid::Liquid;
use crate::ocean::Ocean;
use crate::particles::Particles;
use crate::thermal::Thermal;
use crate::volcano::Volcano;

/// Returns every system in the binding execution order.
pub fn registry() -> Vec<Box<dyn System>> {
    vec![
        Box::new(Granular::default()), Box::new(Liquid::default()), Box::new(AirSystem::default()), Box::new(Thermal),
        Box::new(Humidity), Box::new(Fire::default()), Box::new(Volcano::default()), Box::new(Particles),
        Box::new(Collisions), Box::new(Explosions::default()), Box::new(Chemistry),
        Box::new(Ocean::default()), Box::new(Lighting),
    ]
}
