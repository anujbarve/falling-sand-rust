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
