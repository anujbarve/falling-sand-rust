# sandbox_v2_gpu

Purpose: wgpu device creation and window-surface handling for sandbox v2.

Interfaces:
- `Gpu { instance, adapter, device, queue }`
- `Gpu::new_headless() -> Option<Gpu>` (None when no adapter)
- `Gpu::new_windowed(target, w, h) -> Option<(Gpu, WindowSurface)>`
- `WindowSurface::{resize, size, clear_frame}`; zero-size frames are skipped.
- `CellRaw` (32 bytes, `#[repr(C)]`, bytemuck Pod); `MAX_CELLS = 1 << 20`
- `GridState::new(gpu, w, h) -> Result<_, GridError>` (rejects 0 or > 1024x1024 cells)
- `GridState::{swap, upload, readback, current, other, width, height}`; readback reads the current buffer
- `src/shaders/common.wgsl`: `Cell`, `Dims`, clamped `idx` (not yet dispatched)

Test: `cargo test -p sandbox_v2_gpu` (prints SKIPPED when no GPU adapter).

## Units and timers

- 1 cell = 1 cm (`CELL_M = 0.01`), 1 tick = 1/60 s (`DT`), gravity 9.81 m/s^2 = 981 cells/s^2 (`GRAVITY_CELLS`).
- Timers are written in SECONDS and converted once with `ticks(seconds) = ceil(seconds * 60)` (`ticks(0.5) == 30`).
- `materials::MATERIALS[Material as usize]` holds `MaterialDef { density, terminal_cps, burn_s, evap_s, melt_k, boil_k }`
  (`NEVER = INFINITY` means "does not apply"). `condense_k` is the temperature below which a gas condenses (Steam 373.15 K, else `NEVER`).
  `terminal_cps` drives sand/water; `burn_s`, `evap_s`, `melt_k`, `boil_k`, `condense_k` drive thermal/fire (`density` unused).
- Material ids (0..=17) mirror v1 `sandbox_core::Material` variant for variant; `tests/units.rs::material_ids_match_v1` is the drift test.
- `materials::WALL = 18` is the boundary-solid id. It is a non-material const (not in `MATERIALS`, not a `Material` variant) shared by the reference and shaders.

## Granular and liquid passes (row-parity rule, real-time velocity)

`src/reference.rs` is the CPU reference that DEFINES GPU behaviour for sand and water; the WGSL kernel
(`src/shaders/rules.wgsl`, plus a header generated in `pass_rows.rs`) must match it exactly. This rule INTENTIONALLY
DIFFERS from v1 (no in-row sweep, no `updated` flags, no Rng stream); v1 is not the oracle.

- Per tick: granular (even rows, then odd rows), then liquid (even rows, then odd rows). Gravity is +y.
- Each phase reads only the state at its start. Conflicts for one empty cell: nearest mover straight above, then
  above-left, above-right, then sideways-left, sideways-right. Losers stay and reset their fall state.
- Velocity (integer fixed point, 1 cell = 65536): `vy` field = fall speed (fixed-point cells/tick), `vx` field = sub-cell
  remainder. Each tick: `v = min(vy + GRAVITY_FP, terminal)`, `acc = frac + v`, move `n = acc / 65536` whole cells
  straight down through cells empty at phase start, keep `frac = acc % 65536`. Blocked early, diagonal, sideways or
  lost move: `vy = frac = 0`. `flags` holds `tick + 1` once a mover was considered this tick (so a mover that changes
  row parity is not integrated twice). Max step is `MAX_STEP = 15` cells per tick (900 cells/s).
- Sand: down, else diagonal (hash bit 0). Water: also one sideways step (hash bit 1).
- Free fall: 100 cells (1 m) take 27 ticks = 0.45 s (ideal sqrt(2h/g) = 0.4515 s).
- Hash: `cell_hash(tick, x, y, seed)`, murmur3 fmix32 chain in u32 wrapping math, identical in Rust and WGSL.
- Values are integers stored in the `f32` fields, so velocity state is compared bitwise in the parity test.

API: `GranularPass::new(&gpu)` / `LiquidPass::new(&gpu)` compile once and own a uniform buffer and cached bind groups;
`pass.dispatch(&gpu, &grid, tick, seed)` records both phases in one encoder and leaves the result in the grid's
current buffer. `reference::Reference` is the CPU model.

Parity test: `cargo test -p sandbox_v2_gpu --test parity_granular_liquid -- --nocapture`
(prints the mismatch counts; skips without a GPU). CPU-only: `--test reference`, `--test units`.

## Air (advection, pressure, buoyancy)

`src/air.rs` is the CPU reference (`AirReference`) and constants; `src/shaders/air.wgsl` (7 entry points, header generated in
`pass_air.rs`) must match it. `AirPass::new(&gpu)` compiles once; `pass.dispatch(&gpu, &grid, dt, wind)` records all 26
dispatches in one encoder. (The plan's free function `dispatch_air` is a method because the pass must persist.)

- Layout: vapor and pressure are `CellRaw.vapor` / `CellRaw.pressure`. Air velocity is a SEPARATE per-grid `vec2<f32>` buffer
  owned by `AirPass` (cells/s, wind excluded); `CellRaw.vx/vy` stay granular state. `pass.read_velocity(&gpu, &grid)` reads it.
- Per tick: force (buoyancy `vy -= 3.348*(temp-293)*dt`, i.e. g/293 per K, hot air rises since +y is down; drag 2/s; clamp
  60 cells/s) -> advect velocity (Stam semi-Lagrangian, bilinear) -> divergence -> exactly `AIR_SWEEPS = 20` Jacobi sweeps from
  p = 0 -> subtract gradient -> advect vapor -> store vapor and pressure. Cost never depends on contents (`dispatch_count`).
- Solids (Stone, Rock, Wood, Ash, Metal, Glass, Ice, WALL) have zero air velocity, zero vapor and zero pressure; pressure
  neighbours that are solid or outside use the centre value. Speeds are clamped to 60 cells/s = 1 cell/tick so a backtrace
  cannot jump a one-cell wall (holds for `dt <= DT`).
- `wind` (cells/s) is an ambient velocity added to the transport velocity only. It is blocked by solids but does not flow around them.
- Conservation: vapor is conserved to 1e-3 only for the uniform-wind test (measured leak 1.5e-7). Bilinear weights summing to 1
  does not make semi-Lagrangian advection conservative in general: divergent buoyant flow is NOT a conservation test and may
  gain or lose vapor. Vapor entering a solid is absorbed. No rescale is applied.
- Open boundary: the grid edge is open, not a wall. Backtrace sample positions are clamped to the grid, so the edge cells
  re-sample themselves and vapor/air can pile up or drain there. Use WALL cells for a closed box.
- Sanitizing: NaN/Inf vapor, pressure and air velocity are zeroed at the start of each tick and counted in a debug counter
  (`AirPass::sanitized_count`, test use only).
- Known limits: a one-cell diagonal staircase of solids can leak vapor through its corners; the pass caches buffers for ONE
  grid at a time (dispatching alternately on two grids resets the cached air velocity).
- Parity is float, not bitwise (fma/contraction differ): tolerance 1e-4 (vapor), 1e-3 (pressure, velocity) of the field maximum;
  measured 1.6e-8/1.5e-2, 4.2e-5/27.3, 1.9e-4/60.5 (abs diff / field max).

Tests: `cargo test -p sandbox_v2_gpu --test air -- --nocapture` (CPU-only tests run without a GPU; the rest skip).

## Thermal, fire and lava

`src/reference_thermal.rs` is the CPU reference (`ThermalReference`) and holds every constant and per-material table;
`src/shaders/thermal.wgsl`, `fire.wgsl`, `lava.wgsl` must match it. The tables (edge rates, 1/heat capacity, burn ticks)
are generated into the shader header as exact f32 bit patterns, so both sides use identical numbers. Tick order:
`ThermalPass::dispatch(&gpu, &grid)`, `FirePass::dispatch(&gpu, &grid)`, `LavaPass::dispatch(&gpu, &grid, tick, seed)`
(the plan's `dispatch_thermal/fire/lava` are methods because the passes persist). Each compiles once and caches bind groups per grid.

- Conduction (double-buffered): `T += sum_edges(rate(m, n) * (T_n - T)) / C(m)`, `rate = DT * 2 ka kb / (ka + kb)` (harmonic
  mean = series resistance), outside the grid and WALL are insulating. The edge flux is exactly antisymmetric, so heat
  (`sum C*T`) is conserved; measured relative change 9e-9 (CPU) / 1e-8 (GPU) over 600 ticks, and 2.9e-8 over 300 ticks with
  176 wet wood cells shedding heat into water (asserted < 1e-7). Explicit stability number 0.33 (<= 0.5, tested).
- Conductivity (1/s per edge, hand-compressed scale keeping the real order): air 0.02, steam/smoke 0.02, oil/ash/gunpowder 0.04,
  sand/wood/ember 0.05, water/acid 0.08, glass 0.09, lava 0.1, rock/ice 0.15, stone 0.2, metal 0.5, fire 0.5 (flame convection and
  radiation, not conduction), WALL 0. Air conducts poorly, which is what keeps lava hot.
- Heat capacity (relative to granite, density x specific heat): air 0.01 (real ~6e-4, raised for explicit stability), water 2.0,
  lava 2.2 (includes latent heat of crystallisation), metal 1.7, rock 1.2, stone/glass 1.0, ice 0.9, ... (table in the source).
- Phase changes on the new temperature (`MATERIALS` thresholds, `HYST_K = 2` hysteresis): ice -> water above melt+2; water -> ice
  below melt-2; water boils by LATENT HEAT, not a timer: at 373.15 K it is held there and stores the heat above it,
  `(T-373.15)*C`, in `fuel`; when `fuel >= LATENT_WATER = 1060` (2.257 MJ/kg in granite heat-capacity units) it becomes steam,
  and below 373.15 K stored heat is released first. Water lifetime therefore depends on heat input; `evap_s` is NOT used for
  water (no other material evaporates yet, so `evap_s` is currently unused); steam -> water
  below condense-2; lava -> rock below `LAVA_FREEZE_K = 700`; stone/rock -> lava at `melt_k`; sand -> glass at `melt_k`.
- Moisture (ruling): a flammable cell with an orthogonal water neighbour is capped at 373.15 K, and the excess heat `C*(T-373.15)`
  is MOVED into its water neighbours (split equally), not deleted. Water warms, boils and evaporates; then the wood heats and can
  ignite. Heat is conserved as `sum C*T + sum water fuel` (phase changes change `C` and are outside that statement).
  `IGNITE_K = 573` (wood autoignition, Babrauskas 2003); `FIRE_K = 1200` (wood flame, Drysdale); `LAVA_FREEZE_K = 700` is a GAMEPLAY
  value from v1 (real basalt solidifies near 1250 K).
- Fire (in place): flammable (finite `burn_s`) at >= 573 K ignites: becomes Fire with `fuel = ticks(burn_s)` of the original
  material (wood: 360 ticks = 6 s) and holds `FIRE_K = 1200` while burning; at 0 it becomes Ash. A placed Fire with `fuel < 1`
  is a bare flame and gets `ticks(1.0)`.
- Lava: `fuel` is the viscosity timer. Countdown, then two row-parity phases. Lava falls straight down every tick; it slides
  diagonally only when its timer is 0, and spreads sideways only with lava directly above it (yield stress: a 1-cell sheet
  stops). Every move swaps it with the empty cell and sets the timer to `ticks(LAVA_DELAY_S = 0.25)` = 15 ticks.
- `CellRaw.fuel` is the per-material timer in whole ticks (Fire: burn ticks left; Water: latent heat absorbed,
  heat-capacity units x K; Lava: viscosity delay ticks).
  `flags` is NOT used (it is the granular per-tick stamp).
- Sanitizing: non-finite temp -> 293 K, non-finite fuel -> 0, each counted (`ThermalPass::sanitized_count`); temp clamped to
  0..10 000 K, fuel to 0..1e6.
- Parity: materials, timers and all non-temperature fields bitwise; temperature to 1e-4 of the field max (fma contraction).
  Measured on a mixed 64x64 scene after 300 ticks: 0 / 0 / 0 mismatches, max |dT| 1.2e-4 K of 1974 K (6e-8).
- Measured: 3x3 lava blob at 1500 K on stone in air: all 9 cells above 700 K at 20 s (asserted >= 5), some lava above 700 K
  for > 60 s (hottest 939 K at 60 s); a single lava cell holds 16.7 s (printed, not asserted); wood burns 6.000 s then ash.
  Wet wood (two water neighbours, long-lived flame above): water starts boiling at 4.55 s, the last water turns to steam at
  29.3 s, the wood ignites at 31.15 s (dry: 2.68 s). Asserted: no ignition and both water cells still liquid for 10 s; wood
  <= 373.15 K whenever water touches it.

Binding budget (default limit 8 storage buffers per stage): all three passes share one layout from `pass_cells.rs`:
1 uniform + 3 storage (src cells, dst cells, 4-byte counter). Thermal: 1 dispatch + 1 buffer copy; fire: 1 dispatch; lava: 3
dispatches. For comparison granular/liquid use 2 storage, air uses 8 (at the limit).

Known limits: granular/liquid moves leave a zeroed (0 K) cell behind (Task 3/4 behaviour), which conduction then sees as a cold
spot; fix when the passes are combined in the sim loop. Fire leaves Ash for every fuel (no smoke yet); oil/acid do not boil.

Tests: `cargo test -p sandbox_v2_gpu --test thermal_fire_lava -- --nocapture`.
