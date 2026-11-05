# sandbox_systems

Simulation systems that operate on `sandbox_core` data.

## Modules

- `engine` — the `System` trait and `run_tick` (advance time, clear `updated`, step systems in order; events are left for the app to drain).
- `granular` — `Granular`: sand falls, slides diagonally, and sinks through lighter liquids (density order Sand > Water > Acid > Oil); also hosts the shared density and swap helpers.
- `liquid` — `Liquid`: water, oil and acid fall, slide diagonally, then flow sideways up to 4 cells; acid is only moved, not reacted.
- `air` — `AirSystem { wind }`: semi-Lagrangian advection of velocity and vapor, buoyancy from cell temperature, relaxation toward the background wind, pressure diffusion (Gauss-Seidel, total conserved) with a gradient force, velocity clamped to `MAX_VEL`.
- `thermal` — `Thermal`: explicit 4-neighbour heat diffusion (`CONDUCT = 0.2`, checked <= 0.25, edges insulated, total heat conserved) after applying the `PHASES` table (sand melts to glass, water boils to steam or freezes to ice, ice melts, lava cools to rock). Heat sources (lava, fire) are added by their own systems.
- `humidity` — `Humidity`: Steam cells become 1.0 vapor (only into vapor-free cells), water above `EVAP_TEMP = 350` evaporates (5% per tick), and an empty cell below `DEW_POINT = 285` with vapor > 0.8 becomes Water, gathering 1.0 vapor from itself and its 4-neighbours. One Steam/Water cell = 1.0 vapor, so mass is conserved; vapor is clamped to [0, 1]. Rain is just `Liquid` moving the condensed water.
- `fire` — `Fire`: consumes `Event::Ignite` (other events are left), then each Fire cell heats its 8 neighbours (+40, capped), ignites Wood/Gunpowder neighbours with chance `0.15 * (1 + wind_dot) / (1 + adjacent_water)` (`wind_dot` = neighbour direction dot normalised local wind, clamped to [-0.5, 1]), may release Smoke upward, and turns to Ash (5% Ember if no flammable neighbour remains) when its `fuel` reaches 0 (ignited cells with no fuel get 60).
- `volcano` — `Volcano { sources }` of `VolcanoSource { x, y, rate }`: emits `floor(n*rate) - floor((n-1)*rate)` Lava (temp 1500) per tick into the empty cell above each source (off-grid sources emit nothing), and moves all Lava itself: it falls straight every tick but slides diagonally or sideways (one cell) only every 4th tick. Cooling to rock is `Thermal`'s Lava entry in `PHASES`.
- `particles` — `Particles` and `GRAVITY` (9.81 cells/s^2 downward; all particle units are cells and seconds): semi-implicit Euler (velocity gets gravity, then position uses the new velocity), `life` counts down by `dt`, and particles with `life <= 0` or outside the grid are removed.
- `collisions` — `Collisions`: a particle that ends a move inside a solid cell (Stone, Rock, Wood, Metal, Glass, Ice) is put back at its previous position and bounces with `RESTITUTION = 0.6` on the blocked axis; particle pairs closer than `CONTACT_DISTANCE = 1.0` and approaching swap their normal velocity (equal masses, momentum conserved). All pairs for up to 512 particles, a 1-cell hash bucket above that.
- `explosions` — `Explosions { shocks }`: consumes `Event::Explosion` (other events stay queued). Per blast, cells within `r` get pressure `energy*(1-d/r)^2`, temp 1200, and become Empty (Wood, Oil, Gunpowder become Ash); Sand, Stone, Wood and Metal within `r/2` become debris particles (at most `energy` of them, speed `energy*0.25..0.5` cells/s, radially outward, life 2 s). A `Shock` ring then adds a one-cell-thick pressure band at `r*(age/0.3)` with amplitude `energy*(1-age/0.3)` on later ticks. Writes are clipped to the grid.
- `chemistry` — `Chemistry`, `REACTIONS` (`Reaction { a, b, out_a, out_b, chance, emit }`) and `ReactionEmit { Ignite, Steam }`: every cell is checked against its 4 neighbours in the table (either order); a match rolls `chance` and rewrites both cells. A cell reacts at most once per tick (`updated`). `Ignite` pushes `Event::Ignite` at each cell that becomes Fire (given default fuel); `Steam` is just an output cell. Table: Acid+Metal -> Steam+Empty (1.0), Acid+Stone -> Empty+Empty (0.02), Fire+Gunpowder -> Fire+Fire (0.5, Ignite).
- `ocean` — `Ocean { wind_speed, sea_level }` writes `world.waves` as three summed travelling sines (amplitude scaled by `clamp(wind/5, 0, 1)`, bounded by `AMP_MAX = 6` cells) and moves surface Water above the wave level into the nearest column with room (mass conserved).
- `lighting` — `Lighting`, `RADIUS = 12`: clears `world.light` then, per emitter (Lava, Fire, Ember), adds `1/(1+d²)` to every cell within the radius whose Bresenham line to the emitter crosses no Stone, Rock, Metal or Glass; sums are clamped to [0, 1]. Read-only over cells, work scales with emitters.
- `stubs` — `registry()`, which returns the systems in the fixed order: granular, liquid, air, thermal, humidity, fire, volcano, particles, collisions, explosions, chemistry, ocean, lighting. Granular, liquid, air, thermal, humidity, fire, volcano, particles, collisions, explosions, chemistry, ocean and lighting are real; no stubs remain.

## Tests

```bash
cargo test -p sandbox_systems
```
