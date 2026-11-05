# sandbox_core

Core simulation data types shared by every other sandbox crate.

## Exposes

- `vec2`: `Vec2` with add, sub, scale, `length`, `normalized`.
- `rng`: `Rng`, a seeded xorshift32 (`next_u32`, `next_f32`, `range_f32`, `chance`).
- `grid`: `Grid<T>`, row-major with signed bounds-checked access.
- `material`: `Material` enum of all cell kinds.
- `cell`: `Cell` (material, temperature, fuel, updated marker).
- `field`: `Field` scalar field and `Air` (velocity, pressure, vapor).
- `particle`: `Particle` free-moving particle.
- `event`: `Event` and `SoundKind`, drained by the app each tick.
- `world`: `World`, all layers; `World::new(w, h)` gives an empty 293.0 world.
- `time`: `DT` (1/60 s) and `FixedStep`, capped at 8 ticks per frame.
- `VERSION`: the crate version string.

## Tests

```bash
cargo test -p sandbox_core
```
