//! Physical units of the simulation.
//!
//! One cell is one centimetre, one tick is 1/60 s, gravity is 9.81 m/s^2 =
//! 981 cells/s^2. Timers are written in seconds and converted with [`ticks`].
//!
//! Granular and liquid motion uses integer fixed point so the CPU reference and
//! the GPU agree bit for bit: lengths are in 1/65536 cell ([`FP_ONE`] = one
//! cell) and velocities in fixed-point cells per tick.

/// Edge length of one cell in metres.
pub const CELL_M: f32 = 0.01;
/// Gravity in cells per second squared (9.81 m/s^2 at 1 cell = 1 cm).
pub const GRAVITY_CELLS: f32 = 981.0;
/// Simulation ticks per second.
pub const TICK_HZ: u32 = 60;
/// Duration of one tick in seconds.
pub const DT: f32 = 1.0 / 60.0;

/// One cell in fixed point (16 fractional bits).
pub const FP_ONE: i32 = 1 << 16;
/// Gravity as fixed-point cells per tick gained per tick (`g * DT^2 * 65536`, rounded).
pub const GRAVITY_FP: i32 = (GRAVITY_CELLS * DT * DT * FP_ONE as f32 + 0.5) as i32;
/// Largest whole-cell move per tick. Every terminal velocity must satisfy
/// `terminal_cps * DT <= MAX_STEP` so a move never exceeds this.
pub const MAX_STEP: u32 = 15;

/// Converts a duration in seconds to a whole number of ticks, rounding up.
/// A tiny tolerance keeps values like `0.1` (6.0000005 ticks in f32) at 6.
pub fn ticks(seconds: f32) -> u32 {
    (f64::from(seconds) * f64::from(TICK_HZ) - 1e-3)
        .ceil()
        .max(0.0) as u32
}

/// Converts a speed in cells per second to fixed-point cells per tick (rounded).
pub fn cps_to_fp(cps: f32) -> i32 {
    (cps * DT * FP_ONE as f32 + 0.5) as i32
}
