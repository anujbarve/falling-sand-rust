//! Library half of the v2 app: the fixed-step GPU sim loop ([`frame`]),
//! scenes, controls, save/load in the v1 format, event-to-sound mapping and
//! platform paths. Everything here runs headless, so it is testable without
//! a window; `main.rs` adds the winit window, renderer, audio and HUD.

pub mod controls;
pub mod frame;
pub mod hud;
pub mod platform;
pub mod save;
pub mod scenes;
pub mod sounds;
