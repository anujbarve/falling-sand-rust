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
  Only `terminal_cps` is consumed so far (sand, water); the rest is for thermal/fire.
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
- Vapor is conserved by construction in open air (bilinear weights sum to 1); vapor entering a solid is absorbed. No rescale.
  Test: relative leak 1.5e-7 over 2 s of wind.
- Parity is float, not bitwise (fma/contraction differ): tolerance 1e-4 (vapor), 1e-3 (pressure, velocity) of the field maximum;
  measured 1.6e-8/1.5e-2, 2.5e-5/27.3, 7.8e-5/60.5 (abs diff / field max).

Tests: `cargo test -p sandbox_v2_gpu --test air -- --nocapture` (CPU-only tests run without a GPU; the rest skip).
