//! Events emitted by systems and drained by the app each tick.

/// Category of sound a system can request.
#[derive(Clone, Copy, Debug)]
pub enum SoundKind { Flow, Wind, Crackle, Boom, Hiss, Splash }

/// Something that happened in the world this tick.
#[derive(Clone, Copy, Debug)]
pub enum Event {
    /// An explosion at `(x, y)` with the given radius and energy.
    Explosion { x: f32, y: f32, radius: f32, energy: f32 },
    /// A request to ignite the cell at `(x, y)`.
    Ignite { x: i32, y: i32 },
    /// A sound request; `intensity` is expected in 0..1.
    Sound { kind: SoundKind, intensity: f32, x: f32, y: f32 },
}
