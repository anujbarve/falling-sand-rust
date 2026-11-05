//! Simulation systems that operate on `sandbox_core` data.

pub mod air;
pub mod engine;
pub mod granular;
pub mod humidity;
pub mod liquid;
pub mod thermal;
pub mod stubs;

pub use engine::{run_tick, System};
pub use stubs::registry;
