# sandbox_systems

Simulation systems that operate on `sandbox_core` data.

## Modules

- `engine` — the `System` trait and `run_tick` (advance time, clear `updated`, step systems in order; events are left for the app to drain).
- `stubs` — no-op placeholder systems and `registry()`, which returns them in the fixed order: granular, liquid, air, thermal, humidity, fire, volcano, particles, collisions, explosions, chemistry, ocean, lighting. Later tasks replace one stub each, keeping the name.

## Tests

```bash
cargo test -p sandbox_systems
```
