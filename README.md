# rusty

Learning Rust by building 2D falling-sand simulations.

- `sand/`: falling-sand sim with macroquad. Sand, water, stone, wood, fire, smoke, gunpowder, explosions.
- `learn/`: the AI teaching system ([anujbarve/learn](https://github.com/anujbarve/learn)), as a git submodule. It's wired up for Claude Code, Antigravity and Codex via `AGENTS.md`, `.claude/` and `.agents/`.

```bash
git clone --recursive https://github.com/anujbarve/rusty
cd rusty/sand && cargo run
```

Controls: `1` sand, `2` water, `3` stone, `4` erase, `5` wood, `6` fire, `7` gunpowder. Hold the left mouse button to paint, hold the right mouse button to erase, and press `E` for a bomb. Run the simulation tests with `cargo test`.
