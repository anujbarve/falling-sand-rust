//! Core simulation data types for the sandbox: grid, RNG, world layers and fixed timestep.

pub mod cell;
pub mod event;
pub mod field;
pub mod grid;
pub mod material;
pub mod particle;
pub mod rng;
pub mod time;
pub mod vec2;
pub mod world;

/// Crate version string, taken from `CARGO_PKG_VERSION`; never empty.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
