//! A second falling-sand engine, built so the same rules can run on the CPU
//! and on the GPU (wgpu compute shaders) and be compared fairly.
//!
//! What is more "physical" than the original engine in `main.rs`:
//!
//! - **Density and buoyancy.** Every material has a density. A heavier fluid
//!   sinks through a lighter one, and the bigger the density gap, the faster
//!   it sinks: sand falls fast through air but slowly through water, oil
//!   rises slowly through water, steam shoots up through air.
//! - **Friction and viscosity.** Powders slide down slopes only some of the
//!   time (friction sets the pile angle). Liquids spread at different speeds:
//!   water fast, oil slower, lava very slowly.
//! - **Heat.** Every cell has a temperature. Heat flows between neighbours
//!   by conduction, at a rate set by how conductive both materials are
//!   (air insulates, ice and water conduct well). Gases also lose heat to the
//!   surroundings.
//! - **Phase changes driven by temperature.** Water boils to steam at 100°C,
//!   steam condenses back into rain when it cools, water freezes, ice melts,
//!   lava cools into stone, very hot stone melts back into lava, and wood and
//!   oil catch fire at their ignition points. Fire burns hot and heats its
//!   surroundings, which is how it spreads.
//!
//! How one step works, and why it parallelises:
//!
//! 1. **Heat pass.** Every cell reads its 4 neighbours from the *old* grid and
//!    writes its new temperature and material into a *new* grid. No cell
//!    writes anywhere another cell writes, so all cells can run at once.
//! 2. **Move pass (Margolus blocks).** The grid is cut into 2x2 blocks. Each
//!    block rearranges only its own 4 cells, so all blocks can run at once.
//!    Every other step the block grid is shifted by one cell diagonally, so
//!    material can cross block borders.
//!
//! The original engine scans cells one by one and lets each move anywhere
//! nearby. That only works in order, so it cannot run on a GPU.
//!
//! All the maths is integer maths and the randomness is a hash of
//! (position, step number), not a running generator. So the CPU and the GPU
//! give exactly the same result, bit for bit, which the tests check.

pub mod clock;
pub mod cpu;
pub mod gpu;
pub mod materials;
pub use materials::*;
pub mod rng;
pub use rng::*;
pub mod world;
pub use world::*;
pub mod thermal;
pub use thermal::*;
pub mod motion;
pub use motion::*;
pub mod chunks;
pub use chunks::*;
pub mod scenes;
pub use scenes::*;

#[cfg(test)]
mod tests;
