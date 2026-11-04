//! World time vs. wall time: how many simulation steps to run each frame
//! so that the world plays at a chosen speed (slow motion, real time or
//! faster).
//!
//! One step is `STEP_SECONDS` (1/1200 s) of world time. At speed 1x a frame
//! that took 1/60 s of wall time must run 1/60 s of world time, which is
//! 20 steps. At 0.1x it is 2 steps, and at 0.01x one step every 5 frames:
//! the fractional part carries over to the next frame.
//!
//! If the GPU cannot keep up (too many steps for one frame), the clock runs
//! as many as the budget allows and forgets the rest, instead of falling
//! further and further behind. `achieved` then shows the real speed.

use super::STEPS_PER_SECOND;

/// The speeds the viewer steps through with , and .
pub const SPEEDS: [f64; 11] = [0.01, 0.02, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];
/// Index of 1x in `SPEEDS`.
pub const REAL_TIME: usize = 6;

pub struct Clock {
    /// Index into `SPEEDS`.
    pub speed_index: usize,
    /// World seconds simulated since the last reset.
    pub world_seconds: f64,
    /// Steps owed but not run yet (always below 1 unless behind).
    debt: f64,
    /// Smoothed world seconds per wall second actually achieved.
    pub achieved: f64,
    /// Did the last frame have to drop steps to stay within budget?
    pub behind: bool,
}

impl Default for Clock {
    fn default() -> Self {
        Clock { speed_index: REAL_TIME, world_seconds: 0.0, debt: 0.0, achieved: 0.0, behind: false }
    }
}

impl Clock {
    pub fn speed(&self) -> f64 {
        SPEEDS[self.speed_index]
    }

    pub fn slower(&mut self) {
        self.speed_index = self.speed_index.saturating_sub(1);
    }

    pub fn faster(&mut self) {
        self.speed_index = (self.speed_index + 1).min(SPEEDS.len() - 1);
    }

    pub fn real_time(&mut self) {
        self.speed_index = REAL_TIME;
    }

    /// A new world: time starts again at 0.
    pub fn reset(&mut self) {
        self.world_seconds = 0.0;
        self.debt = 0.0;
    }

    /// Steps to run for a frame that took `wall_seconds`, running at most
    /// `max_steps`. Call with `paused = true` to run nothing (and owe nothing).
    pub fn steps_for_frame(&mut self, wall_seconds: f64, max_steps: u32, paused: bool) -> u32 {
        // A frame longer than 0.1 s (window dragged, debugger) is not
        // "catch up 3 seconds of physics": treat it as 0.1 s.
        let wall = wall_seconds.clamp(0.0, 0.1);
        let steps = if paused {
            self.debt = 0.0;
            0
        } else {
            self.debt += wall * self.speed() * STEPS_PER_SECOND as f64;
            // The epsilon stops 0.2 + 0.2 + ... = 0.99999 from losing a step.
            let whole = (self.debt + 1e-9).floor();
            self.behind = whole > max_steps as f64;
            let run = whole.min(max_steps as f64);
            // Keep the fraction; drop what we could not afford.
            self.debt = if self.behind { 0.0 } else { self.debt - run };
            run as u32
        };
        self.add_steps(steps);
        if wall > 0.0 {
            let now = steps as f64 / STEPS_PER_SECOND as f64 / wall;
            self.achieved = if self.achieved == 0.0 { now } else { self.achieved * 0.9 + now * 0.1 };
        }
        steps
    }

    /// Steps run outside `steps_for_frame` (single-stepping while paused).
    pub fn add_steps(&mut self, steps: u32) {
        self.world_seconds += steps as f64 / STEPS_PER_SECOND as f64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(clock: &mut Clock, frames: usize, wall: f64, max: u32) -> u32 {
        (0..frames).map(|_| clock.steps_for_frame(wall, max, false)).sum()
    }

    #[test]
    fn real_time_at_60_fps_is_20_steps_a_frame() {
        let mut c = Clock::default();
        let total = run(&mut c, 60, 1.0 / 60.0, 10_000);
        // 1 s of wall time = 1 s of world time = 1200 steps (floating point
        // may leave the last step for the next frame).
        assert!((1199..=1200).contains(&total), "{total}");
        assert!((c.world_seconds - 1.0).abs() < 0.002);
        assert!(!c.behind);
    }

    #[test]
    fn slow_motion_carries_fractions_over() {
        let mut c = Clock { speed_index: 0, ..Default::default() }; // 0.01x
        let steps: Vec<u32> = (0..10).map(|_| c.steps_for_frame(1.0 / 60.0, 10_000, false)).collect();
        // 0.01 * 1200 / 60 = 0.2 steps a frame: one step every 5 frames.
        assert_eq!(steps.iter().sum::<u32>(), 2, "{steps:?}");
    }

    #[test]
    fn fast_forward_scales_and_respects_the_budget() {
        let mut c = Clock::default();
        c.faster();
        c.faster(); // 4x
        assert_eq!(c.speed(), 4.0);
        let total = run(&mut c, 60, 1.0 / 60.0, 10_000);
        assert!((4799..=4800).contains(&total), "{total}");
        // A GPU that can only afford 30 steps a frame: run 30, do not pile up debt.
        let n = c.steps_for_frame(1.0 / 60.0, 30, false);
        assert_eq!(n, 30);
        assert!(c.behind);
        assert_eq!(c.steps_for_frame(1.0 / 60.0, 30, false), 30, "no backlog from the dropped steps");
    }

    #[test]
    fn pause_and_long_frames() {
        let mut c = Clock::default();
        assert_eq!(c.steps_for_frame(1.0 / 60.0, 10_000, true), 0);
        // A 2 s hitch counts as 0.1 s: 120 steps, not 2400.
        assert_eq!(c.steps_for_frame(2.0, 10_000, false), 120);
    }
}
