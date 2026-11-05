use sandbox_core::config::{load_config, Config};
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sbx_cfg_{}_{}", std::process::id(), name));
    let _ = std::fs::remove_file(&d);
    d
}

#[test]
fn missing_file_falls_back_with_warning() {
    let (c, w) = load_config(&tmp("missing.toml"));
    assert_eq!(c, Config::default());
    assert!(w.is_some());
}

#[test]
fn malformed_file_falls_back_with_warning() {
    let p = tmp("bad.toml");
    std::fs::write(&p, "wind = = nope [").unwrap();
    let (c, w) = load_config(&p);
    assert_eq!(c, Config::default());
    assert!(w.is_some());
}

#[test]
fn valid_file_loads_without_warning() {
    let p = tmp("ok.toml");
    std::fs::write(&p, "wind = 1.5\n").unwrap();
    let (c, w) = load_config(&p);
    assert_eq!(c.wind, 1.5);
    assert_eq!(c.seed, Config::default().seed);
    assert!(w.is_none());
}

#[test]
fn shipped_default_file_parses_cleanly() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../config/default.toml");
    let (c, w) = load_config(&p);
    assert!(w.is_none(), "{:?}", w);
    assert_eq!(c, Config::default());
}
