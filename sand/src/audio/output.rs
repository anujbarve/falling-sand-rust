//! Opens the sound device and runs the `Engine` in its callback.

use super::engine::Engine;
use super::AudioFrame;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Keeps the audio stream alive. Drop it to stop the sound.
pub struct AudioOut {
    frames: rtrb::Producer<AudioFrame>,
    _stream: cpal::Stream,
    pub description: String,
}

impl AudioOut {
    /// Start sound on the default output device. Fails (instead of
    /// panicking) when there is no usable device; the game then runs silent.
    pub fn start() -> Result<AudioOut, String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("no audio output device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let config = supported.config();
        let (sr, channels) = (config.sample_rate as f32, config.channels as usize);
        // A short queue: the audio thread only ever needs the newest frame.
        let (producer, mut consumer) = rtrb::RingBuffer::<AudioFrame>::new(8);
        let mut engine = Engine::new(sr);
        let stream = device
            .build_output_stream::<f32, _, _>(
                config,
                move |data: &mut [f32], _| {
                    while let Ok(frame) = consumer.pop() {
                        engine.set_frame(frame);
                    }
                    engine.process(data, channels);
                },
                |e| eprintln!("audio stream error: {e}"),
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(AudioOut { frames: producer, _stream: stream, description: format!("{} Hz, {} ch", sr as u32, channels) })
    }

    /// Hand the newest frame to the audio thread. Never blocks: if the queue
    /// is full the audio thread is behind and this frame is dropped.
    pub fn send(&mut self, frame: AudioFrame) {
        let _ = self.frames.push(frame);
    }
}
