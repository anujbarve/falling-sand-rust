# rusty

Learning Rust by building 2D falling-sand simulations.

- `sand/`: falling-sand sim with macroquad. Sand, water, stone, wood, fire, smoke, gunpowder, explosions.
- `learn/`: the AI teaching system ([anujbarve/learn](https://github.com/anujbarve/learn)), as a git submodule. It's wired up for Claude Code, Antigravity and Codex via `AGENTS.md`, `.claude/` and `.agents/`.

Built with [macroquad](https://macroquad.rs). Needs Rust 1.85 or later, because the crate uses the 2024 edition.

```bash
git clone --recursive https://github.com/anujbarve/rusty
cd rusty/sand && cargo run
```

Controls: `1` sand, `2` water, `3` stone, `4` erase, `5` wood, `6` fire, `7` gunpowder. Hold the left mouse button to paint, hold the right mouse button to erase, and press `E` for a bomb. Run the simulation tests with `cargo test`.

## Physics engine: CPU vs GPU (`sand/src/physics`)

A second engine that runs on the CPU and on the GPU (wgpu compute shaders). The physics is closer to the real thing:

- **Density and buoyancy.** Heavier fluids sink through lighter ones. A bigger density gap means faster sinking: sand drops fast through air but slowly through water, and oil floats up.
- **Friction and viscosity.** Each powder and liquid has its own chance to slide or flow. Water spreads fast, oil slower, lava very slowly.
- **Heat.** Every cell has a temperature. Heat flows between neighbours by conduction (air insulates, ice and water conduct well). Hot gases are lighter than cold ones, so they rise (convection).
- **Phase changes.** Water boils into steam, steam rises and condenses into rain, water freezes and ice melts. Lava cools into stone, or into gravel if it cools while falling or sinking in water. Hot stone melts. Wood and oil catch fire at their ignition temperature.

```bash
cd sand
cargo run --release --bin physics   # GPU window: Esc settings, , slower  . faster  / real time  H heat view
cargo run --release --bin bench     # CPU vs GPU timing table; add a number for seconds per measurement
```

Every step is split into two passes that can run in parallel. The heat pass reads the old grid and writes a new one. The move pass shuffles 2x2 blocks, and each block only touches its own 4 cells. The maths is all integers, and the randomness is a hash of position and step. So the CPU and GPU results are identical bit for bit, which `cargo test` checks.

Results on an Apple M5 (10 CPU threads), in ms per step:

| grid | cpu 1 thread | cpu all cores | gpu | gpu + readback | gpu vs 1 thread | gpu vs all cores |
|---|---|---|---|---|---|---|
| 640x360 | 1.876 | 0.390 | 0.085 | 0.298 | 22.2x | 4.6x |
| 1280x720 | 8.201 | 1.572 | 0.300 | 0.702 | 27.4x | 5.2x |
| 1920x1080 | 17.873 | 3.351 | 0.671 | 1.422 | 26.6x | 5.0x |
| 2560x1440 | 32.173 | 5.830 | 1.211 | 2.383 | 26.6x | 4.8x |
| 3840x2160 | 72.465 | 13.117 | 2.768 | 5.389 | 26.2x | 4.7x |

"gpu + readback" means one step plus copying the grid back to the CPU so it can be drawn. That is the cost per frame in the viewer.

### Real units and time control

The physics engine runs in real units: 1 cell = 5 mm (the 640x360 window is 3.2 m x 1.8 m) and 1 step = 1/1200 s of world time.

- **Gravity** is 9.81 m/s². Every cell that can move stores its speed. Things accelerate as they fall: a drop of 1.19 m takes the expected $\sqrt{2h/g}$ = 0.49 s, within 10% (tested). Air resistance is not modelled. Liquids drag what sinks through them: sand settles through water at about 0.1 m/s (tested).
- **Heat** uses real thermal diffusivities (m²/s). Each edge's tiny per-step flow is rounded randomly, so heat is still conserved exactly. Real conduction at this scale is slow: lava sitting on wood takes about 20 s to light it, and a 4 s painted flame does not light a plank.
- **Fire and smoke lifetimes** are in seconds.
- **Viewer clock:** the viewer runs as many steps per frame as the chosen speed needs, from 0.01x slow motion through 1x real time to 16x. If the GPU can't keep up, the HUD shows the speed it actually reached.
- **Bench:** the `gpu max speed` column says how much faster than real time each grid size can run. On the M5: 640x360 ≈ 6.8x, 1920x1080 ≈ 1x, 4K ≈ 0.3x.

### Viewer: GPU rendering and settings

The viewer (`src/bin/physics/`) draws on the GPU too. Each frame, the cells read back from the simulation are uploaded unchanged as an RGBA8 texture, so the CPU does no per-pixel work. GLSL shaders then decode the cell bits and draw:

- material colour with stable per-grain variation
- surface lighting and contact shadows
- liquid depth and surface highlights
- black-body glow of hot matter
- bloom, rendered at 1/2, 1/4 or 1/8 resolution with a separable blur
- tone mapping and a vignette

Settings (press `Esc` or click the toolbar button):

- **Graphics:** quality presets Low, Medium, High and Ultra (changing any option makes it Custom), with each effect on or off, bloom resolution, bloom strength and exposure.
- **Display:** window size (1280x720 to 2560x1440), fullscreen (`F11`), frame cap (default 120 fps; VSync is requested but macOS ignores it), heat view and performance stats.
- **Simulation:** grid size from 320x180 (1.6 m) to 1280x720 (6.4 m), how much GPU time per frame the simulation may use, and skipping still regions.

The toolbar has a material palette, pause, and speed controls `<` `1x` `>`. The mouse never paints through the UI. In steady state the frame loop allocates nothing: the edit and action buffers are reused, and textures and render targets are only rebuilt when the grid or bloom size changes.
