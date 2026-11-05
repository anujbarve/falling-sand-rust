//! Simulation systems that operate on `sandbox_core` data.

pub mod engine;
pub mod stubs;

pub use engine::{run_tick, System};
pub use stubs::registry;
