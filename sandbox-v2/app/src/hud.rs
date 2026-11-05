//! HUD text. The v2 window has no text renderer yet, so the HUD is shown in
//! the window title (updated a few times per second).

use sandbox_v2_gpu::materials::Material;

/// What the HUD shows.
pub struct Hud<'a> {
    /// Rendered frames per second.
    pub fps: f32,
    /// Sim time per tick, ms.
    pub sim_ms: f32,
    /// Whether `sim_ms` is GPU timestamp time (else CPU record+submit time).
    pub sim_gpu: bool,
    /// Occupied particle slots.
    pub particles: u32,
    /// Selected brush material.
    pub material: Material,
    /// Brush radius in cells.
    pub brush: u32,
    /// Whether the sim is paused.
    pub paused: bool,
    /// One-line message (save/load result, device loss, config warning).
    pub notice: Option<&'a str>,
}

/// Formats the HUD as one line, e.g.
/// `Sandbox v2 | 60 fps | sim 1.67 ms gpu | 900 particles | Sand r3`.
pub fn hud_line(h: &Hud<'_>) -> String {
    let mut s = format!(
        "Sandbox v2 | {:.0} fps | sim {:.2} ms {} | {} particles | {:?} r{}",
        h.fps,
        h.sim_ms,
        if h.sim_gpu { "gpu" } else { "cpu" },
        h.particles,
        h.material,
        h.brush
    );
    if h.paused {
        s += " | PAUSED";
    }
    if let Some(n) = h.notice {
        s += " | ";
        s += n;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_has_fps_sim_ms_and_particles() {
        let l = hud_line(&Hud {
            fps: 59.6,
            sim_ms: 1.666,
            sim_gpu: true,
            particles: 900,
            material: Material::Sand,
            brush: 3,
            paused: true,
            notice: Some("saved"),
        });
        assert_eq!(
            l,
            "Sandbox v2 | 60 fps | sim 1.67 ms gpu | 900 particles | Sand r3 | PAUSED | saved"
        );
    }
}
