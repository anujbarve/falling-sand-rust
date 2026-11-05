# sandbox_v2_app

Purpose: the v2 sandbox application. A winit window (1280x800, "Sandbox v2") running the GPU simulation
(`sandbox_v2_gpu`) at a fixed 60 Hz, rendered with `Renderer`, with sound through the v1 audio thread.

Run: `cargo run --release -p sandbox_v2_app [scene]` (from `sandbox-v2/`). Scene: `volcano_island` (default).
`forest`, `ocean_storm`, `explosion_lab` have their config (grid size, timers) but build a placeholder (stone floor)
until Task 10. Test: `cargo test -p sandbox_v2_app` (GPU tests print SKIPPED without an adapter).

## Controls (v1 mapping)

| Key | Action |
|---|---|
| 1..8 | Select Sand, Water, Oil, Stone, Wood, Gunpowder, Fire, Lava |
| `[` / `]` | Brush radius -1 / +1 (1..20 cells) |
| Left mouse | Paint a disc into EMPTY cells only (never deletes) |
| S / L | Save / load `~/.config/sandbox/save_v2.txt` (v1 text format; off on wasm) |
| Space | Pause / resume (painting still applies while paused) |
| Escape | Quit |

HUD (window title, refreshed every 250 ms; there is no text renderer yet): FPS, sim ms per tick (GPU timestamp
`gpu` when the adapter supports `TIMESTAMP_QUERY`, else CPU record+submit time `cpu`), particle slots in use,
brush material and radius, PAUSED, and the last message (save/load result, device loss, config warning) for 4 s.

Config: `config/default.toml` (cwd, then `~/.config/sandbox/default.toml`, then the source tree) read with
`sandbox_core::config::load_config`; `seed` and `brush` are used. A missing/malformed file gives defaults plus one
message. `volume` is NOT applied (sandbox_audio has no volume API); `wind`/`sea_level` are v1 ocean settings and are
not used (scene configs set the air wind in cells/s).

## Loop (`src/frame.rs`)

- `FixedStep`: accumulator of frame time; `DT = 1/60 s`; at most `MAX_TICKS_PER_FRAME = 4` ticks per frame, the
  backlog beyond that is dropped (no spiral of death). Non-finite or negative frame times count as 0.
- `Sim::tick`: ONE encoder, ONE compute pass, ONE submit per tick, in the order brush strokes (mouse + vent),
  granular, liquid, air, thermal, fire, lava, particles. (No chemistry pass exists in v2 yet.) Thermal flips the
  grid's buffers instead of copying.
- The app steps the sim in `about_to_wait`, renders once in `RedrawRequested`. A 0x0 (minimised) or occluded window
  skips rendering only: the sim keeps ticking and the loop waits until the next tick instead of spinning.
- Events: passes count flow, impact, crackle, boom into the grid's GPU counter buffer. Every 6 ticks (100 ms of sim
  time) the tick's encoder copies the counters (and the tick's two timestamps) to a 32-byte staging buffer and clears
  them; the staging buffer is mapped asynchronously and picked up by `Sim::poll` (non-blocking `device.poll(Poll)`).
  A copy still in flight makes the next one skip, so counts carry over, never get lost. Each batch becomes v1
  `Event::Sound`s (flow -> Flow, impact -> Splash, crackle -> Crackle, boom -> Boom; intensity `1 - exp(-n/scale)`)
  sent with `AudioHandle::send_events`, which only `try_send`s: audio never blocks the sim, and a missing device is
  silent.
- Save/load: blocking GPU readback of cells and air velocity, written with `sandbox_core::save::save_world` (v1 text
  format, so v1 can load it). Fuel is a float timer in v2 and a `u8` in v1: it is rounded and clamped to 0..255
  (lossy above 255 ticks: e.g. fuel 600 saves and reloads as 255; fire timers > 4.25 s or water latent heat > 255 are capped);
  `vx`/`vy` fall state and `flags` load as 0; particles are not saved. A load of another
  size resizes the grid. A corrupt or unknown-version file shows the error and leaves the world untouched.
- Known issues:
  - Fuel clamp: v1 format stores `u8`, so v2 float timers above 255.0 are clamped to 255 upon save.
  - Submits are routed through `submit_encoder` to keep submit counting deterministic.
- NaN/inf in uploaded state (scene, load, brush) is replaced and counted (`Sim::sanitized`).
- Device loss: a lost-device callback sets a flag; every GPU call is then skipped (no panic). The app drops the
  surface, creates a new device, surface and renderer, and `Sim::recover` rebuilds all passes and reloads the last
  save (else restarts the scene); the HUD shows the message. If no device is available it retries every frame.

Volcano Island: v1's island scaled to the grid (default 512x512): stone floor, rock cone, crater with a lava pool,
wood trees on the rim and slopes, sea. A vent above the crater paints lava (1500 K) every 2 ticks into empty cells
and throws 3 ember particles; the lava mound grows, touches the rim trees and ignites them.

Dependencies: `sandbox_core` and `sandbox_audio` are the v1 crates as path dependencies (`../../sandbox/*`), used
unchanged for the save format, config loader and audio thread, so v1 and v2 stay compatible.
