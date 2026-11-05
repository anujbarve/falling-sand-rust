//! Every user-tweakable option of the viewer, in one plain struct.
//! `Quality` presets fill in the graphics options; changing one option by
//! hand afterwards makes the preset "Custom".

/// Simulation grid sizes (cells). The world is always shown scaled to fit
/// the window, so a smaller grid means bigger grains and much less GPU work.
pub const GRIDS: [(usize, usize); 5] = [(320, 180), (480, 270), (640, 360), (960, 540), (1280, 720)];
pub const GRID_NAMES: [&str; 5] = [
    "320 x 180 (1.6 x 0.9 m)",
    "480 x 270 (2.4 x 1.35 m)",
    "640 x 360 (3.2 x 1.8 m)",
    "960 x 540 (4.8 x 2.7 m)",
    "1280 x 720 (6.4 x 3.6 m)",
];
pub const DEFAULT_GRID: usize = 2;

/// Window sizes (logical pixels).
pub const WINDOWS: [(f32, f32); 4] = [(1280.0, 720.0), (1600.0, 900.0), (1920.0, 1080.0), (2560.0, 1440.0)];
pub const WINDOW_NAMES: [&str; 4] = ["1280 x 720", "1600 x 900", "1920 x 1080", "2560 x 1440"];

/// Frame rate caps. VSync is requested too, but it is only a hint (macOS's
/// OpenGL ignores it), so a sleep-based cap is the default: drawing frames
/// the display never shows only burns battery and heats the GPU.
pub const FPS_CAPS: [Option<u32>; 4] = [None, Some(30), Some(60), Some(120)];
pub const FPS_CAP_NAMES: [&str; 4] = ["Uncapped", "30 fps", "60 fps", "120 fps"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Quality {
    Low,
    Medium,
    High,
    Ultra,
    Custom,
}

pub const QUALITY_NAMES: [&str; 5] = ["Low", "Medium", "High", "Ultra", "Custom"];

impl Quality {
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn from_index(i: usize) -> Quality {
        [Quality::Low, Quality::Medium, Quality::High, Quality::Ultra, Quality::Custom][i.min(4)]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Graphics {
    /// Glow around hot things (bloom). The costliest effect, but it runs at
    /// a fraction of the screen resolution.
    pub bloom: bool,
    /// Bloom texture size as a fraction of the grid: 1/2, 1/4 or 1/8.
    pub bloom_divisor: u32,
    /// How strongly the glow is added (0..2).
    pub bloom_strength: f32,
    /// Surface lighting: a soft light from above-left makes piles and
    /// liquid surfaces read as shapes, plus contact shadows.
    pub shading: bool,
    /// Liquids get darker with depth.
    pub depth: bool,
    /// Stable per-grain colour variation and liquid shimmer.
    pub grain: bool,
    /// Hot solids and liquids glow by temperature (black-body tint).
    pub incandescence: bool,
    /// Overall exposure (0.5..2).
    pub exposure: f32,
    /// Darken the screen edges a little.
    pub vignette: bool,
}

impl Graphics {
    pub fn preset(q: Quality) -> Graphics {
        let base = Graphics {
            bloom: true,
            bloom_divisor: 4,
            bloom_strength: 1.0,
            shading: true,
            depth: true,
            grain: true,
            incandescence: true,
            exposure: 1.0,
            vignette: true,
        };
        match q {
            Quality::Low => Graphics { bloom: false, shading: false, depth: false, vignette: false, ..base },
            Quality::Medium => Graphics { bloom_divisor: 8, depth: false, ..base },
            Quality::High | Quality::Custom => base,
            Quality::Ultra => Graphics { bloom_divisor: 2, bloom_strength: 1.2, ..base },
        }
    }
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub quality: Quality,
    pub graphics: Graphics,
    pub grid: usize,
    pub window: usize,
    pub fullscreen: bool,
    /// Index into FPS_CAPS.
    pub fps_cap: usize,
    /// Most GPU time per frame the simulation may use (ms). Lower keeps the
    /// frame rate up; the world then runs slower than the chosen speed.
    pub gpu_budget_ms: f32,
    /// Skip regions where nothing can happen (results are identical).
    pub sleeping: bool,
    pub heat_view: bool,
    pub show_stats: bool,
    /// Sound driven by the simulation (on by default; M toggles).
    pub audio: bool,
    /// Master volume 0..1.
    pub volume: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            quality: Quality::High,
            graphics: Graphics::preset(Quality::High),
            grid: DEFAULT_GRID,
            window: 0,
            fullscreen: false,
            fps_cap: 3,
            gpu_budget_ms: 10.0,
            sleeping: true,
            heat_view: false,
            show_stats: true,
            audio: true,
            volume: 0.7,
        }
    }
}

impl Settings {
    /// After the user edits graphics options by hand: is it still a preset?
    pub fn refresh_quality(&mut self) {
        let matching = [Quality::Low, Quality::Medium, Quality::High, Quality::Ultra]
            .into_iter()
            .find(|&q| Graphics::preset(q) == self.graphics);
        self.quality = matching.unwrap_or(Quality::Custom);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_round_trip_and_custom_is_detected() {
        let mut s = Settings::default();
        s.graphics = Graphics::preset(Quality::Low);
        s.refresh_quality();
        assert_eq!(s.quality, Quality::Low);
        s.graphics.bloom = true;
        s.refresh_quality();
        assert_eq!(s.quality, Quality::Custom);
    }
}
