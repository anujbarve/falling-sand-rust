//! Platform isolation: config/save paths and whether file IO exists.
//!
//! Mirrors v1 `app::platform` (that crate is macroquad-based, so it is not
//! a dependency here). Config and save file IO must go through this module.

use std::path::PathBuf;

/// Directory for config and save files: `$XDG_CONFIG_HOME/sandbox`, else
/// `~/.config/sandbox` (Windows: `%APPDATA%\sandbox`; wasm32: `.`).
pub fn config_dir() -> PathBuf {
    if cfg!(target_arch = "wasm32") {
        return PathBuf::from(".");
    }
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.unwrap_or_else(|| PathBuf::from(".")).join("sandbox")
}

/// Whether file IO is available on this target (false on wasm32).
pub fn file_io_available() -> bool {
    !cfg!(target_arch = "wasm32")
}

/// The save file used by `S` and `L` (v1 text format).
pub fn save_path() -> PathBuf {
    config_dir().join("save_v2.txt")
}

/// The config file: `config/default.toml` in the working directory, then in
/// the config dir, then in the source tree. Falls back to the first
/// candidate so the loader reports it as missing.
pub fn config_path() -> PathBuf {
    let rel = PathBuf::from("config").join("default.toml");
    let candidates = [
        rel.clone(),
        config_dir().join("default.toml"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(&rel),
    ];
    candidates
        .iter()
        .find(|p| p.exists())
        .cloned()
        .unwrap_or_else(|| candidates[0].clone())
}
