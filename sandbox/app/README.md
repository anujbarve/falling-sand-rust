# app

The sandbox binary. Opens a macroquad window titled "Sandbox", clears to dark blue, and exits on Escape. Depends on all four library crates.

## Exposes

A binary only; run with `cargo run --release -p app`.

## Tests

```bash
cargo test -p app
```

## Platform module

`app::platform` (`src/platform.rs`) holds all platform `cfg` logic. `config_dir()` returns `$XDG_CONFIG_HOME/sandbox` or `~/.config/sandbox` (macOS/Linux), `%APPDATA%\sandbox` (Windows), and `"."` on wasm32 where file IO is skipped (`file_io_available()` is false). The wasm check is `cargo check --target wasm32-unknown-unknown -p app`.
