//! Audio layer: maps simulation events to sounds and plays them on a dedicated thread.
//!
//! The sim thread only ever calls [`AudioHandle::send_events`], which never blocks
//! and never panics. A missing or unplugged output device silences audio and nothing else.

mod mapping;
mod player;

pub use mapping::{map_event, AudioCmd};
pub use player::AudioHandle;
