//! Platform isolation: all `cfg(target_os)` / `cfg(target_arch)` and config-path logic lives here.
//!
//! Config and save file IO in the app must go through this module.

use std::path::PathBuf;

/// Directory for config and save files.
///
/// Windows: `%APPDATA%\sandbox`. macOS/Linux: `$XDG_CONFIG_HOME/sandbox`, else `~/.config/sandbox`.
/// wasm32: `"."` (callers must skip file IO there; see [`file_io_available`]).
#[cfg(target_arch = "wasm32")]
pub fn config_dir() -> PathBuf {
    PathBuf::from(".")
}

/// Directory for config and save files (Windows: `%APPDATA%\sandbox`).
#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("sandbox")
}

/// Directory for config and save files (`$XDG_CONFIG_HOME` or `~/.config`, then `sandbox`).
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("sandbox")
}

/// Whether file IO is available on this target (false on wasm32).
pub fn file_io_available() -> bool {
    !cfg!(target_arch = "wasm32")
}
