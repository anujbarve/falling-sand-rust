//! The mixer that runs inside the audio callback. No allocation, no locks:
//! everything is created in `Engine::new` and reused.

use super::bank::{Bank, KINDS, VARIANTS};
use super::dsp::{pan_gains, smooth_coef, Pink, Rng, Smooth, Svf, SvfCoef};
use super::{band_pan, AudioFrame, BANDS, BEDS, BED_FIRE, BED_LAVA, BED_SAND, BED_SIZZLE, BED_STEAM, BED_WATER};

/// Samples per control block: parameters, filters and event triggers are
/// updated this often (1.3 ms at 48 kHz).
const BLOCK: usize = 64;
/// One-shots that can sound at once. When full, the oldest is replaced.
pub const MAX_VOICES: usize = 48;

#[derive(Clone, Copy, Default)]
struct Voice {
    active: bool,
    kind: u8,
    variant: u8,
    pos: f32,
    rate: f32,
    gl: f32,
    gr: f32,
}

#[derive(Clone, Copy, Default)]
struct BedVoice {
    svf_l: Svf,
    svf_r: Svf,
    pink_l: Pink,
    pink_r: Pink,
    level: Smooth,
    pan: Smooth,
    bright: Smooth,
    lfo: f32,
}

pub struct Engine {
    sr: f32,
    bank: Bank,
    rng: Rng,
    frame: AudioFrame,
    beds: [BedVoice; BEDS],
    voices: [Voice; MAX_VOICES],
    volume: Smooth,
    pitch: Smooth,
    master: [Svf; 2],
    master_coef: SvfCoef,
    // Per-kind gain range for one-shots: (min, max). Soft and close.
    kind_gain: [(f32, f32); KINDS],
}

impl Engine {
    pub fn new(sample_rate: f32) -> Engine {
        Engine {
            sr: sample_rate,
            bank: Bank::new(sample_rate),
            rng: Rng::new(0xA5D1_0001),
            frame: AudioFrame::default(),
            beds: [BedVoice::default(); BEDS],
            voices: [Voice::default(); MAX_VOICES],
            volume: Smooth::default(),
            pitch: Smooth { value: 1.0 },
            master: [Svf::default(); 2],
            // A gentle 13 kHz low-pass keeps highs silky rather than sharp.
            master_coef: SvfCoef::new(13_000.0, 0.6, sample_rate),
            kind_gain: [
                (0.03, 0.09), // grain
                (0.05, 0.11), // drip
                (0.06, 0.13), // bubble
                (0.05, 0.12), // crackle
                (0.14, 0.24), // plop
                (0.12, 0.2),  // ignite
                (0.05, 0.1),  // tink
            ],
        }
    }

    pub fn set_frame(&mut self, frame: AudioFrame) {
        self.frame = frame;
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    fn trigger(&mut self, kind: usize, band: usize, pitch: f32) {
        let r = &mut self.rng;
        let variant = (r.next_u32() as usize) % VARIANTS;
        let (g0, g1) = self.kind_gain[kind];
        let gain = g0 + (g1 - g0) * r.f();
        let pan = band_pan(band) + r.bi() * (1.0 / BANDS as f32);
        let (gl, gr) = pan_gains(pan);
        let rate = pitch * (1.0 + 0.04 * r.bi());
        let v = Voice { active: true, kind: kind as u8, variant: variant as u8, pos: 0.0, rate, gl: gl * gain, gr: gr * gain };
        // A free voice, or else the one furthest through its sound.
        let slot = match self.voices.iter().position(|v| !v.active) {
            Some(i) => i,
            None => {
                let mut best = 0;
                let mut best_frac = -1.0;
                for (i, v) in self.voices.iter().enumerate() {
                    let len = self.bank.sounds[v.kind as usize][v.variant as usize].len().max(1) as f32;
                    let frac = v.pos / len;
                    if frac > best_frac {
                        best_frac = frac;
                        best = i;
                    }
                }
                best
            }
        };
        self.voices[slot] = v;
    }

    /// Fill `out` (interleaved, `channels` per frame) with sound.
    pub fn process(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let frames = out.len() / channels;
        let block_s = BLOCK as f32 / self.sr;
        let mut done = 0;
        while done < frames {
            let n = BLOCK.min(frames - done);
            self.block(&mut out[done * channels..(done + n) * channels], channels, n, block_s);
            done += n;
        }
    }

    fn block(&mut self, out: &mut [f32], channels: usize, n: usize, block_s: f32) {
        let f = self.frame;
        // Slow motion: sparser, deeper sound (tape-style). Very slow or paused:
        // fade out rather than drone.
        let audible = f.speed >= 0.05;
        let vol_target = if audible { f.volume } else { 0.0 };
        let volume_coef = smooth_coef(0.08, block_s);
        let pitch = self.pitch.step(f.speed.clamp(0.25, 2.0), smooth_coef(0.2, block_s));

        // One-shots: Poisson events at the measured rates (world rates times
        // clock speed = real rates).
        if audible {
            for kind in 0..KINDS {
                for band in 0..BANDS {
                    let mut expected = f.rates[kind][band] * f.speed * block_s;
                    let mut fired = 0;
                    while fired < 6 && self.rng.f() < expected {
                        self.trigger(kind, band, pitch);
                        expected -= 1.0;
                        fired += 1;
                    }
                }
            }
        }

        // Bed filters for this block.
        let level_coef = smooth_coef(0.15, block_s);
        let pan_coef = smooth_coef(0.35, block_s);
        let mut coefs = [SvfCoef::new(1000.0, 0.7, self.sr); BEDS];
        let mut gains = [0.0f32; BEDS];
        let mut pans = [(0.0f32, 0.0f32); BEDS];
        for i in 0..BEDS {
            let target = f.beds[i];
            let bv = &mut self.beds[i];
            let lvl = bv.level.step(target.level, level_coef);
            let pan = bv.pan.step(target.pan, pan_coef);
            let bright = bv.bright.step(target.bright, level_coef);
            bv.lfo += block_s;
            let (cutoff, q, gain, wobble) = match i {
                BED_SAND => (2500.0 + 3500.0 * bright, 0.9, 0.06, 0.0),
                BED_WATER => (500.0 + 1800.0 * bright, 0.6, 0.09, 0.15 * (bv.lfo * 0.7).sin()),
                BED_FIRE => (420.0, 0.7, 0.16, 0.3 * (bv.lfo * 3.8).sin() * (bv.lfo * 0.6).sin()),
                BED_STEAM => (4500.0, 0.7, 0.035, 0.0),
                BED_LAVA => (110.0, 0.8, 0.22, 0.4 * (bv.lfo * 0.9).sin()),
                _ => (6000.0, 1.2, 0.05, 0.0), // sizzle
            };
            coefs[i] = SvfCoef::new(cutoff * pitch, q, self.sr);
            gains[i] = lvl * gain * (1.0 + wobble);
            pans[i] = pan_gains(pan * 0.8);
        }

        for frame in 0..n {
            let vol = self.volume.step(vol_target, volume_coef / BLOCK as f32);
            let (mut l, mut r) = (0.0f32, 0.0f32);
            for i in 0..BEDS {
                if gains[i] <= 1e-5 {
                    continue;
                }
                let (wl, wr) = (self.rng.bi(), self.rng.bi());
                let bv = &mut self.beds[i];
                let (sl, sr) = match i {
                    BED_SAND => (bv.svf_l.process(wl, &coefs[i]).band, bv.svf_r.process(wr, &coefs[i]).band),
                    BED_STEAM => (bv.svf_l.process(wl, &coefs[i]).high, bv.svf_r.process(wr, &coefs[i]).high),
                    BED_SIZZLE => {
                        // Crackly: the hiss is gated by fast random flutter.
                        let gate = if self.rng.f() < 0.35 { 1.0 } else { 0.25 };
                        (bv.svf_l.process(wl * gate, &coefs[i]).band, bv.svf_r.process(wr * gate, &coefs[i]).band)
                    }
                    _ => {
                        let (pl, pr) = (bv.pink_l.next(wl), bv.pink_r.next(wr));
                        (bv.svf_l.process(pl, &coefs[i]).low, bv.svf_r.process(pr, &coefs[i]).low)
                    }
                };
                l += sl * gains[i] * pans[i].0;
                r += sr * gains[i] * pans[i].1;
            }
            for v in self.voices.iter_mut().filter(|v| v.active) {
                let s = &self.bank.sounds[v.kind as usize][v.variant as usize];
                let i = v.pos as usize;
                if i + 1 >= s.len() {
                    v.active = false;
                    continue;
                }
                let frac = v.pos - i as f32;
                let x = s[i] + (s[i + 1] - s[i]) * frac;
                l += x * v.gl;
                r += x * v.gr;
                v.pos += v.rate;
            }
            // Master: silky low-pass, then a soft limiter (never clips).
            let l = self.master[0].process(l, &self.master_coef).low * vol;
            let r = self.master[1].process(r, &self.master_coef).low * vol;
            let soft = |x: f32| x / (1.0 + x.abs());
            let o = &mut out[frame * channels..(frame + 1) * channels];
            o[0] = soft(l);
            if channels > 1 {
                o[1] = soft(r);
                for c in o.iter_mut().skip(2) {
                    *c = 0.0;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::bank::Kind;
    use crate::audio::Bed;

    fn render(engine: &mut Engine, seconds: f32) -> Vec<f32> {
        let mut out = vec![0.0; (48_000.0 * seconds) as usize * 2];
        engine.process(&mut out, 2);
        out
    }

    #[test]
    fn silence_in_silence_out_and_busy_scene_stays_bounded() {
        let mut e = Engine::new(48_000.0);
        e.set_frame(AudioFrame { volume: 1.0, ..Default::default() });
        let quiet = render(&mut e, 0.5);
        assert!(quiet.iter().all(|x| x.abs() < 1e-4), "nothing happening = silence");

        // Everything at full blast: still finite and below full scale.
        let mut f = AudioFrame { volume: 1.0, ..Default::default() };
        for b in f.beds.iter_mut() {
            *b = Bed { level: 1.0, pan: 0.3, bright: 1.0 };
        }
        for k in f.rates.iter_mut() {
            for r in k.iter_mut() {
                *r = 500.0;
            }
        }
        e.set_frame(f);
        let loud = render(&mut e, 1.0);
        assert!(loud.iter().all(|x| x.is_finite() && x.abs() < 1.0));
        let rms = (loud.iter().map(|x| x * x).sum::<f32>() / loud.len() as f32).sqrt();
        assert!(rms > 0.01, "a busy scene is audible (rms {rms})");
        assert!(e.active_voices() <= MAX_VOICES);
    }

    #[test]
    fn pause_fades_out() {
        let mut e = Engine::new(48_000.0);
        let mut f = AudioFrame { volume: 1.0, ..Default::default() };
        f.beds[BED_WATER] = Bed { level: 1.0, pan: 0.0, bright: 0.5 };
        e.set_frame(f);
        render(&mut e, 0.5);
        f.speed = 0.0; // paused
        e.set_frame(f);
        render(&mut e, 0.5);
        let after = render(&mut e, 0.1);
        assert!(after.iter().all(|x| x.abs() < 1e-3), "paused = silent after a short fade");
    }

    #[test]
    fn banks_are_deterministic() {
        let (a, b) = (Bank::new(48_000.0), Bank::new(48_000.0));
        assert_eq!(a.get(Kind::Bubble, 3), b.get(Kind::Bubble, 3));
    }
}
