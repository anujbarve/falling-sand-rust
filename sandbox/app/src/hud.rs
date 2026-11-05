//! Heads-up display text.

/// Formats the performance line, e.g. `"FPS 60  tick 3.2 ms"` for `fps = 60.0`, `tick_ms = 3.2`.
///
/// `fps` is rounded to a whole number and `tick_ms` to one decimal.
pub fn format_line(fps: f32, tick_ms: f32) -> String {
    format!("FPS {fps:.0}  tick {tick_ms:.1} ms")
}
