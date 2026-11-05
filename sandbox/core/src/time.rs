//! Fixed timestep accumulator.

/// Length of one simulation tick in seconds (60 Hz).
pub const DT: f32 = 1.0 / 60.0;

/// Maximum ticks returned by one `push_frame` call (spiral-of-death guard).
const MAX_TICKS_PER_FRAME: u32 = 8;

/// Converts variable frame times into whole fixed ticks. Invariant: `acc < DT` after each call.
pub struct FixedStep {
    acc: f32,
}

impl FixedStep {
    /// Creates a stepper with no accumulated time.
    pub fn new() -> FixedStep {
        FixedStep { acc: 0.0 }
    }

    /// Adds `frame_dt` seconds and returns how many `DT` ticks to run this frame.
    ///
    /// Capped at 8 ticks per frame to avoid a spiral of death; when the cap is hit
    /// the leftover time is discarded.
    pub fn push_frame(&mut self, frame_dt: f32) -> u32 {
        self.acc += frame_dt;
        let mut ticks = 0;
        while self.acc >= DT && ticks < MAX_TICKS_PER_FRAME {
            self.acc -= DT;
            ticks += 1;
        }
        if ticks == MAX_TICKS_PER_FRAME {
            self.acc = 0.0;
        }
        ticks
    }
}

impl Default for FixedStep {
    fn default() -> Self {
        Self::new()
    }
}
