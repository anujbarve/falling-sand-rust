//! Pure translation from simulation [`Event`]s to audio commands.

use sandbox_core::event::{Event, SoundKind};

/// A command for the audio thread.
#[derive(Clone, Copy, Debug)]
pub enum AudioCmd {
    /// Play a short synthesised sound of `kind` at volume `gain` in `(0, 1]`.
    Play {
        /// Which sound to synthesise.
        kind: SoundKind,
        /// Volume in `(0, 1]`.
        gain: f32,
    },
}

/// Gain used for ignition crackles.
const IGNITE_GAIN: f32 = 0.2;
/// Explosion energy at which the boom reaches half volume.
const BOOM_HALF_ENERGY: f32 = 50.0;

/// Maps an event to a sound, or `None` when it would be silent or the value is invalid.
///
/// Explosion gain rises smoothly with energy and never exceeds 1.
pub fn map_event(e: &Event) -> Option<AudioCmd> {
    match *e {
        Event::Explosion { energy, .. } => {
            if !(energy.is_finite() && energy > 0.0) {
                return None;
            }
            let gain = (energy / (energy + BOOM_HALF_ENERGY)).min(1.0);
            (gain > 0.0).then_some(AudioCmd::Play { kind: SoundKind::Boom, gain })
        }
        Event::Ignite { .. } => Some(AudioCmd::Play { kind: SoundKind::Crackle, gain: IGNITE_GAIN }),
        Event::Sound { kind, intensity, .. } => {
            if !(intensity.is_finite() && intensity > 0.0) {
                return None;
            }
            Some(AudioCmd::Play { kind, gain: intensity.min(1.0) })
        }
    }
}
