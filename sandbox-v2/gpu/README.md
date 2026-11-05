# sandbox_v2_gpu

Purpose: wgpu device creation, window-surface handling, GPU simulation passes and the renderer for sandbox v2.

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
  `terminal_cps` drives sand/water; `burn_s`, `melt_k`, `boil_k`, `condense_k` drive thermal/fire (`density` and `evap_s` unused; water boils by latent heat).
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

## Particles (1M-capable GPU particle system)

`src/reference_particles.rs` is the CPU reference (`ParticleReference`, `step_particle`, `Ring`) and DEFINES the rule;
`src/shaders/particles.wgsl` (header generated in `particles.rs`) must match it bit for bit. `Particles::new(&gpu, cap)`
allocates `cap <= Particles::MAX = 1_000_000` slots (32 MB at the max) and compiles the step pipeline once;
`spawn(&gpu, &batch)` uploads at the ring head; `step(&gpu, &grid, dt)` is one dispatch (256 threads per workgroup) against
the grid's current buffer, bind groups cached per grid; `readback`, `count`, `cap`, `buffer`, `sanitized_count`.

- Layout: `ParticleRaw` (32 bytes, `#[repr(C)]`, Pod): `pos [f32;2]` @0, `vel [f32;2]` @8, `mat u32` @16, `temp f32` @20,
  `life f32` @24, `_pad f32` @28. Units cells, cells/s (+y down), K, seconds. `mat`/`temp` are carried, not simulated.
- Cap: a ring buffer. Spawning writes at `head` and wraps; beyond `cap` the OLDEST slots are overwritten and `count` stays
  at `cap` (a batch larger than `cap` keeps its newest `cap` entries). Head and count live on the CPU (spawning is a CPU
  upload); the count reaches the GPU in the params uniform. Dead particles keep their slot until recycled.
- Motion: semi-implicit Euler at `dt = 1/n` s (`n >= 60`, asserted; normally `DT = 1/60`), integer fixed point with
  `FP_ONE = 1024` units per cell: quantize `floor(x*1024)`, kick `vy += round(981*1024/n)`, drift `d = v / n` (truncating),
  speed clamped to +-1800 cells/s per axis (<= 30 cells per tick). Stored back as `fp/1024`, exact in f32, so parity is
  BITWISE (no float tolerance). Grid sides up to `MAX_SIDE = 16384` keep positions exact (asserted in `step`).
- Launch: `ParticleRaw::launched(pos, vel, ..)` stores `vy - g*DT/2` (leapfrog start). With it, a 45 degree launch at
  300 cells/s lands 91.731 cells away (v^2/g = 91.743, error 0.013%); a plain `vel` start is 3.87% short, matching the
  semi-implicit Euler bias `g*DT/(sqrt(2) v)` = 3.85%.
- Collision: the move is walked cell by cell (integer grid DDA, x first on an exact tie). Entering a solid cell (Stone,
  Rock, Wood, Ash, Metal, Glass, Ice, `air::is_solid`) or leaving the grid (outside = WALL) reflects the crossed component as
  `-(v*3)/5` (restitution 0.6), places the particle at the crossing point clamped inside the previous cell and drops the rest
  of the move. No live particle ends a step inside a solid; a 1800 cells/s particle cannot tunnel through a one-cell wall.
  A live particle found outside the grid or inside a solid at the start of a step (e.g. a cell became solid under it) is
  killed.
- Resting contact and friction (Task 8): a particle is SUPPORTED when its fixed-point `y` is the last unit of its cell,
  the cell below is solid or outside, and `|vy| <= REST_VY_CPS = 30` cells/s (a 30 cells/s bounce rises 0.46 cell). A
  supported particle gets `vy = 0` (no gravity kick: the floor's normal force) and Coulomb friction `FRICTION_MU = 0.5`
  (dry stone/granular, 0.4..0.7): `vx` moves toward 0 by `round(mu*g*1024/hz)` (8371 at 60 Hz) per step, never past 0. It
  then moves the full `vx/hz` horizontally (fixes the Task 7 "crawl": a floor hit dropped all but ~1/170 of the move while
  `vx` stayed large). A 300 cells/s slide stops after 89.24 cells in 37 ticks (ideal `v^2/(2 mu g)` = 91.74; the 2.7%
  short is the kick-then-drift bias `v*dt/2`). A dropped particle bounces, rests and stays put. Off a ledge it falls.
- `ParticleRaw::launched` hard-codes `DT = 1/60` for its half kick (documented; a `dt` argument is a carried follow-up).
- Life: seconds directly, `life -= dt` in f32 each step after motion; `<= 0` becomes exactly 0 (dead, skipped). `+inf`
  never expires.
- Sanitizing (Review Focus 8): non-finite `pos`, `vel` or `temp`, or NaN `life`, zeroes the particle (dead, `mat` kept) and
  increments a debug counter (`sanitized_count`).
- Bindings: 1 uniform + 3 storage (particles rw, grid cells read-only, counter). Max storage bindings for this pass: 3 of 8.
  The particle buffer also has VERTEX usage (the renderer draws it as instances).
- Parity with the resting rule: `parity_300_ticks` 0 mismatches of 40 000; `friction_parity_gpu` (2000 sliders and
  droppers, a step, 240 ticks) 0 mismatches, all 2000 at rest at the end.
- Measured: 1 000 000 particles spawn, one step + 32 MB readback 30 ms (debug test build), bitwise equal to the reference;
  parity over 300 ticks (40 000 slots, mid-run spawn wraps the ring, NaN and expiring particles): 0 mismatches; 20 000
  particles at random speeds through an obstacle course for 300 ticks: 0 in-solid endings (CPU every tick, GPU every 10).

Tests: `cargo test -p sandbox_v2_gpu --test particles -- --nocapture`.

## Rendering: lighting, shading, bloom

`src/render.rs` holds every look constant (documented with sources) and `Renderer`; shaders `light.wgsl`, `shade.wgsl`,
`scene.wgsl`, `bloom.wgsl`, `composite.wgsl` (+ shared `look.wgsl`, `fullscreen.wgsl`) receive them via a generated header.
`Renderer::new(&gpu, target_format)` builds every pipeline once; `renderer.frame(&gpu, &grid, &particles, &target_view)`
records one encoder and one submit. Grid-sized textures and bind groups are rebuilt only when the grid changes. The
render is NOT parity-checked: `tests/render_golden.rs` is the spec. Same input gives identical output bytes (tested).

Look rules, per frame:
1. Light (cell resolution). `emit`: blackbody emission of each cell (below). `jump`: jump flood (Rong & Tan 2006), steps
   `LIGHT_STEPS = [16, 8, 4, 2, 1, 1]`: each cell keeps two candidate emitters, the BRIGHTEST (`luma(E) * falloff(d)`) and the
   NEAREST, within `LIGHT_RADIUS = 32` cells. `resolve`: for both, `E * falloff(d)`, zero if a solid cell lies on the
   straight line to the emitter (ray-marched occluder test, <= 32 steps): hard shadows. Light = max of the two and the cell's
   own emission. `falloff(d) = clamp(1 - (d/32)^4, 0, 1)^2 / (1 + (d/LIGHT_D0)^2)`, `LIGHT_D0 = 6` (windowed inverse square,
   Karis 2013): round glows that reach exactly 0 at the radius (an earlier max-flood gave square glows with a hard edge).
2. Shade (cell resolution, HDR `rgba16float`). Albedo table (linear, per material) times `1 +- amp * noise`, noise =
   `cell_hash(0, x, y, NOISE_SEED)`; lit by `AMBIENT 0.55 + LIGHT_GAIN 0.35 * light`. Liquids (water, oil, acid, lava):
   Beer-Lambert depth darkening over up to `DEPTH_MAX = 12` liquid cells above, water absorbs `(0.30, 0.10, 0.03)` per
   cell (red first, so deep water is blue), others 0.08; the top liquid cell gets a highlight
   `SURF_RGB * 0.8 * max(n . SUN_DIR, 0)^16` with the surface normal from the neighbour columns' surface heights
   (searched +-4 cells). Gases (empty, steam, smoke, fire): background gradient + `AIR_GLOW 0.04 * light`, overlaid by
   vapor/steam/smoke with `alpha = 1 - exp(-2 * vapor)` (Steam cells >= 0.55, Smoke >= 0.75), premultiplied and blurred
   3x3 (binomial) for soft edges, alpha varied by noise 0.85..1.15. Fire emission flickers spatially (0.6..1.0 by noise).
3. Emission: cells hotter than `EMIT_MIN_K = 800` K (Draper point 798 K) glow; colour from a blackbody table (Mitchell
   Charity, vendian.org, CIE 1931 2 deg, sRGB -> linear): 800 K pure red, 1000 K (1, .040, 0), 1200 K (1, .087, 0),
   1500 K (1, .153, 0), 2000 K (1, .254, .006), 3000 K (1, .456, .147), 5000 K (1, .776, .617), 6500 K (1, .947, .982):
   dim red -> orange -> yellow-white. Intensity `2.5 * (T/1200)^4` (Stefan-Boltzmann) faded in by smoothstep 800..1000 K,
   capped at 32.
4. Scene (`scene_scale = clamp(2048 / max(w, h), 1, 4)` pixels per cell, HDR). Sharpened bilinear upsample: flat inside a
   cell, blended over `EDGE_PX = 2` pixels across each cell boundary (no hard pixel blocks). Particles: ONE instanced draw
   of every occupied slot (cap `PARTICLE_DRAW_CAP = Particles::MAX = 1 000 000`), triangle strip quads read straight from
   the particle buffer as a per-instance vertex buffer; dead slots collapse off screen. Cold particles: round grains of
   radius 0.6 cell in albedo x light; hotter than 800 K: soft glow `(1-d)^2` of radius 1.5 cells + blackbody emission.
5. Bloom: bright pass `c * max(Y - 1, 0) / Y` at quarter scene resolution (4 bilinear taps per 4x4 block), separable
   Gaussian sigma 2 texels, radius 4 (9 taps), horizontal then vertical.
6. Composite: `scene + 0.6 * bloom`, exposure 1, ACES filmic fit (Narkowicz 2015,
   `x(2.51x+0.03)/(x(2.43x+0.59)+0.14)`), clamp 0..1, non-finite -> 0; non-sRGB targets get the sRGB transfer in the shader.

Binding budget (storage buffers / storage textures per stage; limit 8): light emit 1/2, light jump 0/1, light resolve 1/1,
shade 1/1, scene fill 0/0, sprites 0/0 (vertex buffer), bloom 0/0, composite 0/0. Dispatches per frame: 1 emit + 6 jump +
1 resolve + 1 shade (compute), then 5 render passes (scene, down, blur h, blur v, composite).

Measured (golden 64x64 -> 256x256 `Rgba32Float`): lava cell Y 0.901 vs surrounding air 0.273 (margin 0.628, asserted
> 0.3); deep water rgb (0.003, 0.128, 0.508), blue-dominant; air 8 cells behind a stone wall Y 0.088 vs open air at the same
distance 0.203 (ratio 0.434, asserted < 0.8); 0 non-finite or out-of-range components; identical bytes across repeated
frames and fresh renderers. Ramp: 700 K no glow, 1000 / 1500 / 3000 K lava Y 0.61 / 0.90 / 1.00 with g/r 0.56 / 0.92 / 1.00.

Known limits: each cell is lit by at most two emitters (brightest and nearest), so a third visible emitter can be missed;
shadows are hard and cell-stepped (light is per cell); the scene stretches to the target (no letterbox); render time at
1024x1024 not yet measured (Task 10).

Tests: `cargo test -p sandbox_v2_gpu --test render_golden -- --nocapture`.
