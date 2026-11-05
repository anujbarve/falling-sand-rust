# sandbox_core

Core simulation data types shared by every other sandbox crate.

## Exposes

- `vec2`: `Vec2` with add, sub, scale, `length`, `normalized`.
- `rng`: `Rng`, a seeded xorshift32 (`next_u32`, `next_f32`, `range_f32`, `chance`).
- `chunk`: `Chunks` (`size` 16, `active`, `w`, `h` in chunks) with `new(width, height)`, `mark(x, y)` (that chunk only), `is_active(cx, cy)`, `clear()`, `mark_all()` and `spread_from(&src)` (src grown by one chunk in all 8 directions).
- `grid`: `Grid<T>`, row-major with signed bounds-checked access.
- `material`: `Material` enum of all cell kinds.
- `cell`: `Cell` (material, temperature, fuel, updated marker).
- `field`: `Field` scalar field and `Air` (velocity, pressure, vapor).
- `particle`: `Particle` free-moving particle.
- `event`: `Event` and `SoundKind`, drained by the app each tick.
- `world`: `World`, all layers; `World::new(w, h)` gives an empty 293.0 world.
- `time`: `DT` (1/60 s) and `FixedStep`, capped at 8 ticks per frame.
- `config`: `Config` (seed, wind, sea_level, brush, volume) and `load_config(path)`; a missing or bad file gives defaults plus one warning string. See `config/default.toml`.
- `save`: `save_world` (atomic: temp file then rename), `load_world`, `SaveError`. Text format `version 1`, `size W H`, cell lines, air lines; unknown version or truncation returns `Err`.
- `VERSION`: the crate version string.

## Tests

```bash
cargo test -p sandbox_core
```
