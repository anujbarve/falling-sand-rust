# Sandbox

Cargo workspace for the sandbox project.

| Crate | Purpose |
|---|---|
| `core` (`sandbox_core`) | Shared core types |
| `systems` (`sandbox_systems`) | Simulation systems |
| `render` (`sandbox_render`) | Rendering helpers |
| `audio` (`sandbox_audio`) | Audio layer |
| `app` | Binary: window and main loop |

Run: `cargo run --release -p app`

Test everything: `cargo test --workspace`

## Platforms and CI

Tests per OS (same command everywhere; Linux needs `sudo apt-get install libasound2-dev` first):

- macOS: `cargo test --workspace`
- Linux: `cargo test --workspace`
- Windows (PowerShell): `cargo test --workspace`

Web (wasm) check: `rustup target add wasm32-unknown-unknown && cargo check --target wasm32-unknown-unknown -p app`
Web bundle (see macroquad's web instructions): `cargo build --release --target wasm32-unknown-unknown -p app`, then serve the `.wasm` with macroquad's `mq_js_bundle.js` and an HTML page.

CI (`.github/workflows/ci.yml`) builds and tests on macOS, Linux and Windows and runs the wasm check.
