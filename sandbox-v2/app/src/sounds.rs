//! Turns one 100 ms batch of GPU event counts into v1 audio [`Event`]s.

use sandbox_core::event::{Event, SoundKind};
use sandbox_v2_gpu::{EventKind, EVENT_KINDS};

/// Counts per batch at which each kind reaches `1 - 1/e` (63 %) loudness,
/// in [`EventKind`] order: flow, impact, crackle, boom.
pub const HALF_SCALE: [f32; EVENT_KINDS] = [400.0, 40.0, 4.0, 1.0];

/// The sound of each [`EventKind`].
pub fn sound_kind(kind: EventKind) -> SoundKind {
    match kind {
        EventKind::Flow => SoundKind::Flow,
        EventKind::Impact => SoundKind::Splash,
        EventKind::Crackle => SoundKind::Crackle,
        EventKind::Boom => SoundKind::Boom,
    }
}

/// One `Sound` event per non-zero count, intensity `1 - exp(-n / scale)` in
/// `(0, 1)`, at the origin (v1 audio ignores position).
pub fn sounds(counts: [u32; EVENT_KINDS]) -> Vec<Event> {
    let kinds = [
        EventKind::Flow,
        EventKind::Impact,
        EventKind::Crackle,
        EventKind::Boom,
    ];
    kinds
        .into_iter()
        .filter(|&k| counts[k as usize] > 0)
        .map(|k| Event::Sound {
            kind: sound_kind(k),
            intensity: 1.0 - (-(counts[k as usize] as f32) / HALF_SCALE[k as usize]).exp(),
            x: 0.0,
            y: 0.0,
        })
        .collect()
}
