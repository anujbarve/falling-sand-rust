# app

The sandbox binary and its testable library half. Depends on all four library crates.

## Run

```bash
cargo run --release -p app                 # volcano island (default)
cargo run --release -p app -- forest       # or ocean_storm, explosion_lab
```

Config is read once at start from `config/scenes/<scene>.toml` (looked up in the working directory, then the platform config dir, then the source tree). A missing or malformed file falls back to defaults and the warning is shown in the HUD.

## Controls

| Input | Action |
|---|---|
| Left mouse | Paint the selected material |
| 1 to 8 | Sand, Water, Oil, Stone, Wood, Gunpowder, Fire, Lava |
| `[` / `]` | Smaller / larger brush (1 to 20 cells) |
| S / L | Save / load `save.txt` in the platform config dir (not on wasm) |
| Space | Pause |
| Escape | Quit |

The HUD shows `FPS n  tick x.y ms`, the scene, brush, material and pause state, and any config or save/load notice.

## Library API

- `app::scenes`: `Scene`, `build(scene, seed) -> World`, `systems_for(scene)`, `systems_with(scene, wind, sea_level)`.
- `app::controls`: `map_key` (pure), `Controls`, `paint`.
- `app::hud`: `format_line(fps, tick_ms)`.
- `app::platform`: config paths and `file_io_available()` (all platform `cfg` logic).

The main loop: pump keys to actions, `FixedStep::push_frame` gives the tick count, `run_tick` per tick, events go to `AudioHandle::send_events` and are then cleared, then `FrameBuffer` and `MacroquadPresenter` draw, then the HUD.

## Tests

```bash
cargo test -p app                                   # scenes, controls, HUD, determinism, soak
cargo test -p app --test soak -- --nocapture        # prints mean tick time and solid drift per scene
```

The workspace `[profile.test]` is optimised (`opt-level = 3`) so the 4 x 10 000 tick soak finishes in about 15 s.

## Platform module

`app::platform` (`src/platform.rs`) holds all platform `cfg` logic. `config_dir()` returns `$XDG_CONFIG_HOME/sandbox` or `~/.config/sandbox` (macOS/Linux), `%APPDATA%\sandbox` (Windows), and `"."` on wasm32 where file IO is skipped (`file_io_available()` is false; `main` gates config loading and S/L on it). The wasm check is `cargo check --target wasm32-unknown-unknown -p app`.
