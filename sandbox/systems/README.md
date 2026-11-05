# sandbox_systems

Simulation systems that operate on `sandbox_core` data.

## Modules

- `engine` — the `System` trait and `run_tick` (advance time, clear `updated`, step systems in order; events are left for the app to drain).
- `granular` — `Granular`: sand falls, slides diagonally, and sinks through lighter liquids (density order Sand > Water > Acid > Oil); also hosts the shared density and swap helpers.
- `liquid` — `Liquid`: water, oil and acid fall, slide diagonally, then flow sideways up to 4 cells; acid is only moved, not reacted.
- `air` — `AirSystem { wind }`: semi-Lagrangian advection of velocity and vapor, buoyancy from cell temperature, relaxation toward the background wind, pressure diffusion (Gauss-Seidel, total conserved) with a gradient force, velocity clamped to `MAX_VEL`.
- `thermal` — `Thermal`: explicit 4-neighbour heat diffusion (`CONDUCT = 0.2`, checked <= 0.25, edges insulated, total heat conserved) after applying the `PHASES` table (sand melts to glass, water boils to steam or freezes to ice, ice melts, lava cools to rock). Heat sources (lava, fire) are added by their own systems.
- `stubs` — no-op placeholder systems and `registry()`, which returns them in the fixed order: granular, liquid, air, thermal, humidity, fire, volcano, particles, collisions, explosions, chemistry, ocean, lighting. Granular, liquid, air and thermal are real; later tasks replace one remaining stub each, keeping the name.

## Tests

```bash
cargo test -p sandbox_systems
```
