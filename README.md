# rusty

Learning Rust by building 2D falling-sand simulations.

- `sand/`: falling-sand sim with macroquad. Sand, water, stone, wood, fire, smoke, gunpowder, explosions.
- `learn/`: the AI teaching system ([anujbarve/learn](https://github.com/anujbarve/learn)), as a git submodule. It's wired up for Claude Code, Antigravity and Codex via `AGENTS.md`, `.claude/` and `.agents/`.

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
cargo run --release --bin physics   # window: G switches cpu 1 thread / cpu all cores / gpu, H shows heat
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
