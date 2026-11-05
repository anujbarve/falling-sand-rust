# Sandbox v2: GPU Compute, Real-Time Scale, Visual Pass

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the 160×120 CPU cell sandbox into a real-time GPU sandbox: a grid up to 1024×1024 cells, 100k–1M particles, physically timed motion (1 cell = 1 cm, seconds-based timers), and a visual pass that reads as water, smoke, fire and lava rather than blocks.

**Architecture:** The sim state lives in GPU storage buffers (`material`, `temp`, `vel`, `fuel`, `vapor`, `pressure`, double-buffered). Each system is a compute pass written in WGSL, dispatched in the same fixed order as the CPU registry. Particles (spray, debris, steam droplets, embers) are a separate GPU particle buffer of up to 1M entries, integrated in compute and drawn as instanced quads. Rendering is a wgpu render graph: cell texture → palette and per-cell shading → lighting → bloom → particle pass → tone map. CPU keeps input, scene loading, config, save/load (via readback), and a compact event readback (once per 100 ms) that drives audio.

**Tech Stack:** wgpu (pinned, same as Task 19), WGSL, bytemuck (reinstated: now needed for GPU buffers), pollster (already present), winit via macroquad is NOT used for the GPU path; a winit window + wgpu surface replaces macroquad for the v2 app. The CPU crates remain as the reference implementation and the test oracle.

**Spec:** The syllabus conversation and the v1 plan (`2026-11-05-sandbox-full-build.md`). Rulings in `.superpowers/sdd/2026-11-05-sandbox-full-build/progress.md` carry forward.

## Global Constraints

- Sim tick is fixed at 1/60 s. A compute frame runs as many ticks as the accumulator allows, capped at 4 per rendered frame.
- Physical scale: 1 cell = 1 cm (`CELL_M = 0.01`). Gravity 9.81 m/s² becomes 981 cells/s². All velocities are in cells/s; terminal velocities per material are in cells/s.
- Every timer is in seconds, converted to ticks by `ceil(seconds / DT)`: fire burn time, lava viscosity delay, evaporation time, condensation time, rain fall time.
- Grid: default 512×512 (262 144 cells). Maximum 1024×1024 (1 048 576 cells). Particle cap 1 000 000.
- Frame budget at 1024×1024 on the development Mac: sim ≤ 10 ms GPU time, render ≤ 4 ms GPU time, measured with timestamp queries. Report the number; a miss is a logged known issue, not silent.
- Determinism: GPU RNG is a hash of (tick, cell index, seed). Same seed and inputs give identical state (verified by readback comparison in tests).
- No unsafe outside the wgpu/bytemuck boundary and the test allocator.
- The CPU reference crates (`sandbox_core`, `sandbox_systems`) are NOT deleted. The GPU port is checked against the CPU oracle on small grids (see V3 parity tests).
- Commit after every task. Conventional Commits. No push without user approval.

## Review Focus

1. Window resize to 0×0 or minimise: no panic, sim keeps stepping, render resumes.
2. Brush, explosion or vent at the grid edge: no out-of-bounds write (GPU bounds check in every shader; test with edge placements).
3. Config file missing or malformed: defaults with one on-screen message.
4. Save file corrupted or unknown version: error shown, world untouched.
5. Audio device missing: sim runs silently.
6. GPU device lost: recreate device, reload last saved state if any, show message; never crash.
7. Particle count at cap: oldest particles recycled, no unbounded growth.
8. NaN or inf in any GPU float buffer: clamped in the shader and counted in a debug counter.

## Tasks

### Task 1: GPU device and window scaffold

**Files:**
- Create: `sandbox-v2/Cargo.toml` (workspace), `sandbox-v2/gpu/` (crate: device, buffers, pipelines), `sandbox-v2/app/` (winit window + wgpu surface)
- Test: `sandbox-v2/gpu/tests/device.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub struct Gpu { device, queue, adapter }` with `pub fn new_headless() -> Option<Gpu>`; app opens a window and clears to a colour each frame; surface reconfigures on resize and skips zero-size frames.

- [ ] **Step 1:** Failing test: `new_headless()` returns `Some` on a machine with an adapter; on none it returns `None` and the test prints SKIPPED. Run, show RED.
- [ ] **Step 2:** Implement device creation (`pollster::block_on`), surface setup and resize handling.
- [ ] **Step 3:** App opens a 1280×800 window, clears, exits on Escape. Run `cargo run --release -p app`.
- [ ] **Step 4:** Commit `feat(gpu): device and window scaffold`.

### Task 2: GPU grid state and cell layout

**Files:**
- Create: `sandbox-v2/gpu/src/state.rs`, `sandbox-v2/gpu/src/layout.rs`, `sandbox-v2/gpu/src/shaders/common.wgsl`
- Test: `sandbox-v2/gpu/tests/state.rs`

**Interfaces:**
- Consumes: `Gpu`.
- Produces: `pub struct GridState { w: u32, h: u32, bufs: [Buffers; 2], cur: usize }` with `pub fn new(gpu: &Gpu, w: u32, h: u32) -> GridState`, `pub fn swap(&mut self)`, `pub fn upload(&self, gpu: &Gpu, cells: &[CellRaw])`, `pub fn readback(&self, gpu: &Gpu) -> Vec<CellRaw>`. `#[repr(C)] pub struct CellRaw { mat: u32, temp: f32, fuel: f32, vx: f32, vy: f32, vapor: f32, pressure: f32, flags: u32 }` (32 bytes).

- [ ] **Step 1:** Failing test: upload 64×64 random cells, readback equals upload byte-for-byte. Also: `w*h > 1<<20` is rejected. RED.
- [ ] **Step 2:** Implement double buffers with `bytemuck` casts; `common.wgsl` defines the `Cell` struct with the same 32-byte layout and a bounds-checked `idx(x, y)`.
- [ ] **Step 3:** GREEN. Commit `feat(gpu): double-buffered grid state`.

### Task 3: Port the granular and liquid rules to WGSL, with CPU parity

**Files:**
- Create: `sandbox-v2/gpu/src/pass_granular.rs`, `sandbox-v2/gpu/src/pass_liquid.rs`, `sandbox-v2/gpu/src/shaders/granular.wgsl`, `sandbox-v2/gpu/src/shaders/liquid.wgsl`
- Test: `sandbox-v2/gpu/tests/parity_granular_liquid.rs`

**Interfaces:**
- Consumes: `GridState`, the CPU reference `sandbox_systems::granular::Granular` and `liquid::Liquid` (as oracle).
- Produces: `pub fn dispatch_granular(gpu, grid, tick, seed)`, `pub fn dispatch_liquid(gpu, grid, tick, seed)`.

- [ ] **Step 1:** Parity test: 128×128 scene with sand column and water basin, 300 ticks, GPU result equals CPU oracle state within the rules' exact outputs (cell materials equal; no floats involved here). RED (no shader yet).
- [ ] **Step 2:** Implement the row-parity update in WGSL: even and odd rows alternate so each pass has no write conflicts (replaces the CPU `updated` flags). Hash-based RNG from (tick, x, y, seed) identical to the CPU hash.
- [ ] **Step 3:** GREEN with exact equality. Commit `feat(gpu): granular and liquid compute passes with CPU parity`.

### Task 4: Real-time units and per-material timers

**Files:**
- Create: `sandbox-v2/gpu/src/units.rs`, `sandbox-v2/gpu/src/materials.rs`
- Modify: `sandbox-v2/gpu/src/pass_granular.rs`, `pass_liquid.rs` (use terminal velocity and gravity from units)
- Test: `sandbox-v2/gpu/tests/units.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub const CELL_M: f32 = 0.01; pub const GRAVITY_CELLS: f32 = 981.0; pub fn ticks(seconds: f32) -> u32;` and `pub struct MaterialDef { pub density: f32, pub terminal_cps: f32, pub burn_s: f32, pub evap_s: f32, pub melt_k: f32, pub boil_k: f32 }` with a `const MATERIALS: [MaterialDef; N]` table in material enum order.

- [ ] **Step 1:** Test: a sand grain dropped from 1 m (100 cells) reaches the floor in 0.45 s ± 0.03 s (free fall: t = sqrt(2·h/g) = sqrt(200/981) ≈ 0.45 s), not in a tick-count guess. Test: `ticks(0.5) == 30`. RED.
- [ ] **Step 2:** Implement: granular and liquid move at most `min(terminal, gravity·t)` cells per tick using an accumulated fractional velocity per cell (stored in `vx`, `vy`). Replace the one-cell-per-tick rule.
- [ ] **Step 3:** GREEN; re-run V3 parity with the new rule as a new oracle (update the CPU reference to the same units so the oracle matches).
- [ ] **Step 4:** Commit `feat(gpu): physical units and per-material timers`.

### Task 5: Gas, advection, pressure and buoyancy on GPU

**Files:**
- Create: `sandbox-v2/gpu/src/pass_air.rs`, `sandbox-v2/gpu/src/shaders/air_advect.wgsl`, `air_pressure.wgsl`, `air_buoy.wgsl`
- Test: `sandbox-v2/gpu/tests/air.rs`

**Interfaces:**
- Consumes: `GridState`, units from V4.
- Produces: `pub fn dispatch_air(gpu, grid, dt, wind: [f32; 2])`.

- [ ] **Step 1:** Tests: a puff of vapor at (64,64) with wind (30, 0) cells/s moves its centre of mass 30·t cells after t seconds ±10% over 2 s; total vapor conserved within 1e-3 relative; no NaN after 10 000 ticks. RED.
- [ ] **Step 2:** Implement semi-Lagrangian advection (Stam), a fixed-count Jacobi pressure solve (20 sweeps, as the CPU Jacobi ruling), and buoyancy `vy += k·(temp − 293)`. Solids block air: air velocity is zeroed on solid cells (this closes the v1 known issue).
- [ ] **Step 3:** GREEN. Commit `feat(gpu): air advection, pressure and buoyancy; solids block air`.

### Task 6: Thermal, phase changes, fire, lava with physical timers

**Files:**
- Create: `sandbox-v2/gpu/src/pass_thermal.rs`, `pass_fire.rs`, `pass_lava.rs`, and shaders `thermal.wgsl`, `fire.wgsl`, `lava.wgsl`
- Test: `sandbox-v2/gpu/tests/thermal_fire_lava.rs`

**Interfaces:**
- Consumes: `GridState`, `MATERIALS`, `ticks()`.
- Produces: `pub fn dispatch_thermal(...)`, `pub fn dispatch_fire(...)`, `pub fn dispatch_lava(...)`.

- [ ] **Step 1:** Tests: lava emitted at 1500 K stays above 700 K for at least 20 s on a stone floor in air (fixes v1 known issue 2 physically: air conducts poorly); fire on wood burns for `burn_s` of wood (6 s) ±0.5 s and produces ash; wet wood does not ignite in 10 s with two water neighbours. RED.
- [ ] **Step 2:** Implement heat diffusion with per-material conductivity (air 0.02, stone 0.2, metal 0.5), phase changes with `melt_k`/`boil_k`, fire fuel countdown in seconds via `ticks()`, lava viscosity as a per-cell delay in seconds.
- [ ] **Step 3:** GREEN. Commit `feat(gpu): thermal, fire and lava with physical timers`.

### Task 7: Particles: 1M-capable GPU particle system

**Files:**
- Create: `sandbox-v2/gpu/src/particles.rs`, `sandbox-v2/gpu/src/shaders/particles.wgsl`
- Test: `sandbox-v2/gpu/tests/particles.rs`

**Interfaces:**
- Consumes: `GridState` (for collision against solid cells).
- Produces: `pub struct Particles { cap: u32, buf: wgpu::Buffer, count: AtomicU32-as-buffer }` with `pub fn new(gpu, cap: u32)`, `pub fn spawn(gpu, batch: &[ParticleRaw])`, `pub fn step(gpu, grid, dt)`, `pub const MAX: u32 = 1_000_000;`. `#[repr(C)] pub struct ParticleRaw { pos: [f32;2], vel: [f32;2], mat: u32, temp: f32, life: f32, _pad: f32 }` (32 bytes).

- [ ] **Step 1:** Tests: spawning 1 000 000 particles succeeds and one step runs; spawning beyond `cap` recycles the oldest (count stays at cap); a projectile's range matches v²/g within 1% in cells/s units. RED.
- [ ] **Step 2:** Implement compute integration with gravity from units, life countdown, collision against the grid (solid cell → reflect with restitution 0.6), ring-buffer recycle.
- [ ] **Step 3:** GREEN. Commit `feat(gpu): million-particle compute system`.

### Task 8: Lighting, shading and bloom

**Files:**
- Create: `sandbox-v2/gpu/src/pass_light.rs`, `sandbox-v2/gpu/src/render.rs`, shaders `light.wgsl`, `shade.wgsl`, `bloom.wgsl`, `composite.wgsl`
- Test: `sandbox-v2/gpu/tests/render_golden.rs`

**Interfaces:**
- Consumes: `GridState`, `Particles`.
- Produces: `pub struct Renderer` with `pub fn frame(&mut self, gpu, grid, parts, target: &wgpu::TextureView)`.

- [ ] **Step 1:** Golden test: a 64×64 scene (water basin, lava cell, one particle) renders to an offscreen texture; the lava cell's luminance exceeds the surrounding air by a fixed margin; the water cell has a blue-dominant hue; output has no NaN and is within 0..1 after tone map. RED.
- [ ] **Step 2:** Implement: per-cell shading from neighbour heights (water surface normal from the top-water column), per-cell colour noise (hash), emission from temp above 800 K (blackbody ramp), 2-pass separable bloom at quarter resolution, tone map (ACES-like), alpha-blended smoke and steam by vapor density.
- [ ] **Step 3:** GREEN. Commit `feat(gpu): lighting, shading and bloom`.

### Task 9: Sim loop, events to audio, app wiring

**Files:**
- Modify: `sandbox-v2/app/src/main.rs`; create `sandbox-v2/app/src/frame.rs`
- Test: `sandbox-v2/app/tests/loop.rs`

**Interfaces:**
- Consumes: all of the above, `sandbox_audio::AudioHandle`, `sandbox_core::{load_config, save_world?}` (save via GPU readback to the v1 text format for compatibility).
- Produces: fixed-step loop: accumulator → up to 4 ticks per frame → passes in order (granular, liquid, air, thermal, fire, lava, chemistry, particles) → render. Events (flow, impact, boom, crackle) are counted in a GPU counter buffer and read back every 100 ms into `AudioHandle::send_events`.

- [ ] **Step 1:** Test: a headless 10 s run (600 ticks) on 256×256 with Volcano Island produces non-zero flow and crackle event counts and no NaN; the loop never runs more than 4 ticks per frame. RED.
- [ ] **Step 2:** Implement loop, counters, readback schedule, device-loss recovery (recreate `Gpu`, reload last state).
- [ ] **Step 3:** GREEN. Controls and HUD carried over from v1 (keys 1–8, `[ ]`, S/L, Space, Escape; HUD shows FPS, sim ms, particle count).
- [ ] **Step 4:** Commit `feat(app): gpu sim loop with event readback to audio`.

### Task 10: Scenes at scale, soak and benchmark

**Files:**
- Create: `sandbox-v2/config/scenes/*.toml` (grid size, particle budget, timers); modify `sandbox-v2/app/src/scenes.rs`
- Test: `sandbox-v2/app/tests/soak_gpu.rs`
- Bench: `sandbox-v2/gpu/benches/frame.rs`

**Interfaces:**
- Consumes: V9 loop.
- Produces: scenes at 512×512 (default) and 1024×1024 (`-- big`).

- [ ] **Step 1:** Soak: each scene runs 10 000 ticks, no NaN, no device loss, particle count ≤ cap; record GPU frame time from timestamp queries. RED where the scene is missing.
- [ ] **Step 2:** Add scenes and the bench. Measure 1024×1024 frame time; report the number honestly against the Global Constraints budget.
- [ ] **Step 3:** GREEN. Commit `feat(app): scaled scenes, soak and gpu benchmark`.

## Final verification

```bash
cd /Users/anujbarve/Documents/rusty/sandbox-v2
cargo test --workspace
cargo run --release -p app            # default 512x512
cargo run --release -p app -- big    # 1024x1024
```

Expected: tests pass (GPU tests skip with a message when no adapter). The default scene runs at 60 fps on the development Mac, or the miss is a Known issue with the measured number.

## Self-Review

- Spec coverage: 100k–1M particles (V7), real-time units (V4), per-material timers (V4, V6), GPU compute (V2–V7), visuals (V8), ASMR events to audio (V9), scale (V10), solids blocking air (V5), lava cooling fix (V6).
- Interfaces are fixed in each task; later tasks consume earlier signatures only.
- Known risks: the V3 parity oracle depends on the CPU reference being updated in V4; V8 visual quality is judged by golden tests plus a manual look, not by pixel-perfect reference.
