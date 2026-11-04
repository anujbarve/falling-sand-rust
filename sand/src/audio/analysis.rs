//! From cells to sound parameters: one pass over the grid per frame.
//!
//! It runs on the main thread on the readback the viewer already has, so it
//! adds no GPU work. One linear pass with a few neighbour reads: about 1 ms
//! for 640x360 cells. Its buffers are allocated once.

use super::bank::Kind;
use super::{band_pan, AudioFrame, Bed, BANDS, BEDS, BED_FIRE, BED_LAVA, BED_SAND, BED_SIZZLE, BED_STEAM, BED_WATER};
use crate::physics::*;

#[derive(Default, Clone, Copy)]
struct Counts {
    sand_moving: u32,
    sand_speed: u32,
    grains_landing: u32,
    water_moving: u32,
    water_speed: u32,
    drops_landing: u32,
    boiling: u32,
    fire: u32,
    fuel_near_fire: u32,
    steam: u32,
    lava: u32,
    quench: u32,
}

pub struct Analyzer {
    prev_fire: [u32; BANDS],
    prev_lava: [u32; BANDS],
    first: bool,
}

impl Default for Analyzer {
    fn default() -> Self {
        Analyzer { prev_fire: [0; BANDS], prev_lava: [0; BANDS], first: true }
    }
}

/// Loudness from an amount: square root (sound power adds, so many sources
/// get louder slowly), scaled so `full` cells give level 1.
fn level(amount: f32, full: f32) -> f32 {
    (amount / full).sqrt().min(1.0)
}

/// Counts are scaled to this grid size, so the same scene sounds the same
/// on every grid setting.
const REFERENCE_CELLS: f32 = 640.0 * 360.0;

impl Analyzer {
    /// Forget the history (a new world was loaded).
    pub fn reset(&mut self) {
        self.first = true;
    }

    /// Measure `world`. `frame_seconds` is the world time since the last
    /// call (for rates of change), `speed` the clock speed (0 when paused).
    pub fn analyze(&mut self, world: &World, frame_seconds: f32, clock_speed: f32, volume: f32) -> AudioFrame {
        let (w, h) = (world.width, world.height);
        let mut c = [Counts::default(); BANDS];
        let cells = &world.cells;
        for y in 0..h {
            let row = y * w;
            for x in 0..w {
                let cell = cells[row + x];
                let m = material(cell);
                if m == AIR || m == STONE {
                    continue;
                }
                let b = x * BANDS / w;
                let below = (y + 1 < h).then(|| material(cells[row + w + x]));
                let landing = below.is_some_and(|bm| KIND[bm as usize] != GAS) && speed(cell) > 0;
                match m {
                    SAND => {
                        if speed(cell) > 0 {
                            c[b].sand_moving += 1;
                            c[b].sand_speed += speed(cell);
                            c[b].grains_landing += landing as u32;
                        }
                    }
                    WATER => {
                        if temp(cell) > celsius(100) {
                            c[b].boiling += 1;
                        }
                        if speed(cell) > 0 {
                            c[b].water_moving += 1;
                            c[b].water_speed += speed(cell);
                            c[b].drops_landing += landing as u32;
                        }
                    }
                    FIRE => {
                        c[b].fire += 1;
                        let next_to = |j: usize| matches!(material(cells[j]), WOOD | OIL);
                        let fuel = (x > 0 && next_to(row + x - 1))
                            || (x + 1 < w && next_to(row + x + 1))
                            || (y + 1 < h && next_to(row + w + x));
                        c[b].fuel_near_fire += fuel as u32;
                    }
                    // A jet's exhaust burns: it roars with the fire bed.
                    EXHAUST => c[b].fire += 1,
                    STEAM => c[b].steam += 1,
                    LAVA => {
                        c[b].lava += 1;
                        let water = |j: usize| material(cells[j]) == WATER;
                        let touches = (x > 0 && water(row + x - 1))
                            || (x + 1 < w && water(row + x + 1))
                            || (y > 0 && water(row - w + x))
                            || (y + 1 < h && water(row + w + x));
                        c[b].quench += touches as u32;
                    }
                    _ => {}
                }
            }
        }

        let mut frame = AudioFrame { speed: clock_speed, volume, ..Default::default() };
        let scale = REFERENCE_CELLS / (w * h).max(1) as f32;
        // Beds: level, pan (weighted centre of where it happens), brightness.
        let bed = |amount: &dyn Fn(&Counts) -> u32, full: f32, bright: f32| {
            let total: u32 = c.iter().map(amount).sum();
            let pan = if total == 0 {
                0.0
            } else {
                (0..BANDS).map(|b| band_pan(b) * amount(&c[b]) as f32).sum::<f32>() / total as f32
            };
            Bed { level: level(total as f32 * scale, full), pan, bright }
        };
        let mean_speed = |moving: u32, speed_sum: u32| if moving == 0 { 0.0 } else { speed_sum as f32 / moving as f32 / 255.0 };
        let sand_moving: u32 = c.iter().map(|k| k.sand_moving).sum();
        let sand_speed: u32 = c.iter().map(|k| k.sand_speed).sum();
        let water_moving: u32 = c.iter().map(|k| k.water_moving).sum();
        let water_speed: u32 = c.iter().map(|k| k.water_speed).sum();
        let mut beds = [Bed::default(); BEDS];
        beds[BED_SAND] = bed(&|k| k.sand_moving, 6000.0, mean_speed(sand_moving, sand_speed));
        beds[BED_WATER] = bed(&|k| k.water_moving, 10000.0, mean_speed(water_moving, water_speed));
        beds[BED_FIRE] = bed(&|k| k.fire, 800.0, 0.5);
        beds[BED_STEAM] = bed(&|k| k.steam, 8000.0, 0.5);
        beds[BED_LAVA] = bed(&|k| k.lava, 16000.0, 0.5);
        beds[BED_SIZZLE] = bed(&|k| k.quench, 600.0, 0.5);
        frame.beds = beds;

        // One-shot rates (per world second). The constants turn "cells doing
        // X right now" into "audible events per second", tuned by ear to be
        // detailed but never a wall of noise.
        let dt = frame_seconds.max(1e-3);
        for b in 0..BANDS {
            let k = &c[b];
            let n = |count: u32| count as f32 * scale;
            // Streams: moving sand is many grains touching (most of the
            // sound), landings add a few more; flowing water is many small
            // bubbles; lava on water fries.
            frame.rates[Kind::Grain as usize][b] = (n(k.sand_moving) * 0.12 + n(k.grains_landing) * 1.0).min(140.0);
            frame.rates[Kind::Drip as usize][b] = (n(k.water_moving) * 0.02 + n(k.drops_landing) * 0.4).min(30.0);
            frame.rates[Kind::Bubble as usize][b] = (n(k.boiling) * 0.3).min(20.0);
            frame.rates[Kind::Crackle as usize][b] = (n(k.fuel_near_fire) * 1.5 + n(k.fire) * 0.12).min(25.0);
            frame.rates[Kind::Plop as usize][b] = (n(k.lava) * 0.002).min(1.5);
            frame.rates[Kind::Fry as usize][b] = (n(k.quench) * 1.0).min(90.0);
            if !self.first {
                let ignited = k.fire.saturating_sub(self.prev_fire[b]);
                let set = self.prev_lava[b].saturating_sub(k.lava);
                frame.rates[Kind::Ignite as usize][b] = (n(ignited) / dt * 0.03).min(3.0);
                frame.rates[Kind::Tink as usize][b] = (n(set) / dt * 0.03).min(5.0);
            }
            self.prev_fire[b] = k.fire;
            self.prev_lava[b] = k.lava;
        }
        self.first = false;
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_world_is_silent_and_pouring_sand_is_heard() {
        let mut a = Analyzer::default();
        let still = World::new(64, 64);
        let f = a.analyze(&still, 1.0 / 60.0, 1.0, 1.0);
        assert!(f.beds.iter().all(|b| b.level == 0.0), "a still world makes no sound");

        let mut pour = World::new(64, 64);
        for x in 20..40 {
            for y in 10..30 {
                pour.cells[y * 64 + x] = pack(SAND, 120, AMBIENT); // falling grains
            }
        }
        let f = a.analyze(&pour, 1.0 / 60.0, 1.0, 1.0);
        assert!(f.beds[BED_SAND].level > 0.0);
        assert!(f.beds[BED_SAND].pan.abs() < 0.2, "sand in the middle is centred");
    }
}
