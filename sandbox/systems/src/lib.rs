//! Simulation systems that operate on `sandbox_core` data.

pub mod air;
pub mod collisions;
pub mod engine;
pub mod explosions;
pub mod fire;
pub mod granular;
pub mod humidity;
pub mod liquid;
pub mod particles;
pub mod thermal;
pub mod stubs;
pub mod volcano;

pub use engine::{run_tick, System};
pub use stubs::registry;
