# sandbox_systems

Simulation systems that operate on `sandbox_core` data.

## Modules

- `engine` — the `System` trait and `run_tick` (advance time, clear `updated`, step systems in order; events are left for the app to drain).
- `granular` — `Granular`: sand falls, slides diagonally, and sinks through lighter liquids (density order Sand > Water > Acid > Oil); also hosts the shared density and swap helpers.
- `liquid` — `Liquid`: water, oil and acid fall, slide diagonally, then flow sideways up to 4 cells; acid is only moved, not reacted.
- `stubs` — no-op placeholder systems and `registry()`, which returns them in the fixed order: granular, liquid, air, thermal, humidity, fire, volcano, particles, collisions, explosions, chemistry, ocean, lighting. Granular and liquid are real; later tasks replace one remaining stub each, keeping the name.

## Tests

```bash
cargo test -p sandbox_systems
```
