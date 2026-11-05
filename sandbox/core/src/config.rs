//! Runtime configuration loaded from a TOML file, falling back to defaults.

use serde::Deserialize;
use std::path::Path;

/// Tunable settings. Any field missing from the file keeps its default.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Seed for the simulation RNG.
    pub seed: u32,
    /// Wind speed driving ocean waves.
    pub wind: f32,
    /// Row of the resting ocean surface.
    pub sea_level: i32,
    /// Brush radius in cells.
    pub brush: u32,
    /// Master sound volume, 0.0 to 1.0.
    pub volume: f32,
}

impl Default for Config {
    fn default() -> Config {
        Config { seed: 1, wind: 2.0, sea_level: 40, brush: 3, volume: 0.5 }
    }
}

/// Loads the config at `path`. Never fails: a missing, unreadable or malformed
/// file yields `Config::default()` and `Some(warning)`; success yields `None`.
pub fn load_config(path: &Path) -> (Config, Option<String>) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            let w = format!("config {}: cannot read ({e}); using defaults", path.display());
            return (Config::default(), Some(w));
        }
    };
    match toml::from_str::<Config>(&text) {
        Ok(c) => (c, None),
        Err(e) => {
            let w = format!("config {}: invalid ({e}); using defaults", path.display());
            (Config::default(), Some(w))
        }
    }
}
