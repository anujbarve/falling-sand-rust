//! Platform path tests.

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn config_dir_ends_with_sandbox() {
    assert!(app::platform::config_dir().ends_with("sandbox"));
}

#[cfg(target_arch = "wasm32")]
#[test]
fn config_dir_is_current_dir_on_wasm() {
    assert_eq!(app::platform::config_dir(), std::path::PathBuf::from("."));
}
