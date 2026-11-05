//! The audio thread, its bounded command queue and the synthesised sounds.

use crate::mapping::{map_event, AudioCmd};
use rodio::buffer::SamplesBuffer;
use sandbox_core::event::{Event, SoundKind};
use std::f32::consts::TAU;
use std::num::NonZero;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

/// Maximum commands waiting for the audio thread; further commands are dropped.
const QUEUE_CAP: usize = 64;
/// Sample rate of the synthesised buffers.
const RATE: u32 = 22_050;

/// Sim-side handle to the audio thread. Cheap to hold; sending never blocks.
pub struct AudioHandle {
    tx: SyncSender<AudioCmd>,
}

impl AudioHandle {
    /// Spawns the audio thread and returns its handle.
    ///
    /// If no output device can be opened (or the thread cannot be spawned) the
    /// receiving side is dropped, so the handle holds a dead channel and every
    /// send is silently discarded.
    pub fn start() -> AudioHandle {
        let (tx, rx) = sync_channel(QUEUE_CAP);
        // On spawn failure the closure (and `rx`) is dropped: dead channel.
        let _ = std::thread::Builder::new().name("audio".into()).spawn(move || run(rx));
        AudioHandle { tx }
    }

    /// Maps and queues events for playback. Never blocks and never panics;
    /// commands are dropped when the queue is full or audio is unavailable.
    pub fn send_events(&self, events: &[Event]) {
        for cmd in events.iter().filter_map(map_event) {
            if self.tx.try_send(cmd).is_err() {
                // Full: drop the rest of this batch cheaply. Disconnected: no audio.
                break;
            }
        }
    }
}

/// Audio thread body: owns the output stream; exits (dropping `rx`) if the device fails.
fn run(rx: Receiver<AudioCmd>) {
    let Ok(mut sink) = rodio::DeviceSinkBuilder::open_default_sink() else {
        return;
    };
    sink.log_on_drop(false);
    let mut seed = 0x9E37_79B9u32;
    while let Ok(AudioCmd::Play { kind, gain }) = rx.recv() {
        let samples = synth(kind, gain, &mut seed);
        let (Some(ch), Some(rate)) = (NonZero::new(1u16), NonZero::new(RATE)) else {
            return;
        };
        sink.mixer().add(SamplesBuffer::new(ch, rate, samples));
    }
}

/// Cheap xorshift noise in `[-1, 1)`.
fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Synthesises a short mono buffer for `kind` at the given gain.
fn synth(kind: SoundKind, gain: f32, seed: &mut u32) -> Vec<f32> {
    let (secs, decay) = match kind {
        SoundKind::Boom => (0.6, 6.0),
        SoundKind::Crackle => (0.04, 60.0),
        SoundKind::Flow | SoundKind::Wind => (0.15, 8.0),
        SoundKind::Hiss => (0.25, 7.0),
        SoundKind::Splash => (0.12, 20.0),
    };
    let n = (RATE as f32 * secs) as usize;
    let mut low = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let env = (-decay * t).exp();
            let s = match kind {
                SoundKind::Boom => (TAU * 60.0 * t).sin() * 0.7 + noise(seed) * 0.3,
                SoundKind::Crackle => noise(seed),
                SoundKind::Flow | SoundKind::Wind => {
                    low += 0.1 * (noise(seed) - low);
                    low * 2.0
                }
                SoundKind::Hiss | SoundKind::Splash => noise(seed),
            };
            s * env * gain * 0.5
        })
        .collect()
}
