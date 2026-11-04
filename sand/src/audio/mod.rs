//! ASMR-style audio driven by the simulation (plan: docs/plans/audio.md).
//!
//! Data flow, once per rendered frame:
//!
//! 1. `analysis::Analyzer` reads the cells the viewer already got back from
//!    the GPU and measures what is happening, in 8 bands from left to right:
//!    how much sand is pouring and landing, water flowing and dripping, water
//!    boiling, fire, steam, lava, lava touching water, ignitions, lava
//!    setting into rock. The result is a small fixed-size `AudioFrame`.
//! 2. The frame goes to the audio thread through a lock-free ring buffer
//!    (`rtrb`): the game never waits for audio and audio never waits for the
//!    game.
//! 3. `engine::Engine` (inside the audio callback) turns it into sound:
//!    six continuous procedural "beds" (sand hiss, water flow, fire roar,
//!    steam hiss, lava rumble, sizzle) and short one-shots from `bank`
//!    (grain ticks, drips, bubbles, crackles, plops, whooshes, tinks),
//!    triggered at the measured rates, panned to where they happen.
//!
//! Everything on the audio thread is allocation-free. Event rates and pitch
//! follow the world clock: slow motion plays sparser, deeper sound.
//!
//! The style is close and soft (ASMR): quiet beds, small dry transients,
//! detailed stereo, no harsh highs (a gentle master low-pass and soft limiter).

pub mod analysis;
pub mod bank;
pub mod dsp;
pub mod engine;
pub mod output;

/// Left-to-right bands the screen is cut into for panning.
pub const BANDS: usize = 8;

/// The continuous beds, in `AudioFrame::beds` order.
pub const BED_SAND: usize = 0;
pub const BED_WATER: usize = 1;
pub const BED_FIRE: usize = 2;
pub const BED_STEAM: usize = 3;
pub const BED_LAVA: usize = 4;
pub const BED_SIZZLE: usize = 5;
pub const BEDS: usize = 6;

/// One continuous sound layer: how loud, where, how bright (0..1 each,
/// pan -1..1).
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Bed {
    pub level: f32,
    pub pan: f32,
    pub bright: f32,
}

/// Everything the audio thread needs for one frame. Plain data, `Copy`, so it
/// can go through the ring buffer without allocating.
#[derive(Clone, Copy, Debug)]
pub struct AudioFrame {
    pub beds: [Bed; BEDS],
    /// One-shot event rates in events per *world* second, per kind, per band.
    pub rates: [[f32; BANDS]; bank::KINDS],
    /// World seconds per real second (the clock speed); 0 when paused.
    pub speed: f32,
    /// Master volume 0..1 (0 = muted).
    pub volume: f32,
}

impl Default for AudioFrame {
    fn default() -> Self {
        AudioFrame { beds: [Bed::default(); BEDS], rates: [[0.0; BANDS]; bank::KINDS], speed: 1.0, volume: 0.0 }
    }
}

/// Pan position of a band's centre.
pub fn band_pan(b: usize) -> f32 {
    (b as f32 + 0.5) / BANDS as f32 * 2.0 - 1.0
}
