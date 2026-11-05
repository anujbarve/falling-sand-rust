# Sandbox Full Build Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build one realistic, good-looking falling-sand sandbox in Rust that runs sand, water, air, steam, heat, fire, forests, volcanoes, projectiles, collisions, explosions, chemistry, ocean waves, lighting, and sound together, with a WGPU render path and cross-platform builds.

**Architecture:** A Cargo workspace with three library crates and one app. `core` holds the world data (grid of cells, air field, particles, events) and shared utilities (Vec2, Rng, fixed timestep). `systems` holds one module per physical system, each implementing one `System` trait and each registered in one fixed registry. `render` draws the world to a pixel buffer (macroquad texture first, WGPU behind a feature flag). `audio` turns simulation events into sound on its own thread. `app` wires them together, loads config, and runs scenarios. All systems share one `World` struct whose layers are created once in Task 2, so later tasks only add files and never change the shape of `World`.

**Tech Stack:** Rust stable (latest installed toolchain), `macroquad` (window, input, texture; pin the exact version recorded in `Cargo.lock` at Task 1 and do not upgrade mid-build), `serde` + `toml` (config), `rodio` (audio), `rayon` (parallel bands, Task 17), `criterion` (benchmarks, Task 17), `wgpu` + `bytemuck` (Task 18, behind feature `gpu`), GitHub Actions (Task 19).

**Spec:** No separate spec document exists. This plan is built from the syllabus in the planning conversation (Parts 1–10). The "Global Constraints" and "Review Focus" sections below carry the spec's requirements that the tasks must honor. If a spec document is written later, it must be linked here.

**Location:** Workspace root is `/Users/anujbarve/Documents/rusty/sandbox/`. The old `archive/project` is not modified.

## Global Constraints

- Simulation tick is fixed at `DT = 1.0 / 60.0` seconds. Rendering is not tied to the tick.
- Every random choice goes through `core::Rng` seeded from the scenario seed. No `thread_rng`, no system time in sim code. Same seed and same inputs produce byte-identical worlds.
- Mass is conserved per layer: total solid/liquid cell count and total air mass stay within `1e-3` relative drift per 1000 ticks in the golden tests.
- No `f32` value in sim state may be `NaN` or infinite after any tick. Any system that produces one clamps or resets it and emits a debug warning.
- No `unsafe` anywhere except the optional Task 17 SIMD path, and only behind a safe wrapper with a test.
- Audio never blocks the simulation thread. Events cross to audio through `std::sync::mpsc` channels only.
- Config values come from `config/*.toml`. Missing or malformed config falls back to compiled defaults and logs one line.
- Scenes are saved with a format version number as the first line of the file. Unknown version returns `Err`, never panics.
- Target platforms: macOS (primary), Linux, Windows, and `wasm32-unknown-unknown` (browser). Platform code is isolated behind `cfg` attributes in `app` only.
- Frame-time budget for the default scene (256×256 grid, 1 volcano, 1 forest, 1 ocean strip): sim under 8 ms per tick on the development Mac in release mode. Checked in Task 17 benchmarks.
- Commit after every task. Commit messages use Conventional Commits (`feat:`, `test:`, `refactor:`, `perf:`, `chore:`).

## Review Focus

1. Window resized to zero size, minimized, or moved between displays: no panic, sim keeps stepping, render resumes.
2. Brush, explosion, or volcano placed on or beyond the grid edge: no out-of-bounds write, effect clipped at the border.
3. `config/*.toml` missing, empty, or malformed: app starts with defaults and shows one message on screen.
4. Save file corrupted, truncated, or from an unknown version: load returns an error shown on screen, current world untouched.
5. Audio device absent or unplugged mid-run: sim keeps running silently, no panic, one warning.

Each line above has its test in the owning task (Tasks 16, 15, 20, 19, and 4 respectively; see the step lists).

---

## Shared Interfaces (frozen in Tasks 2 and 4)

These names and signatures are fixed. Any later task that needs a change must edit the interface in Task 2 or 4 first and update every consumer in the same commit.

```rust
// core/src/vec2.rs
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 { pub x: f32, pub y: f32 }
// impl Add, Sub, Mul<f32>; fn length(self) -> f32; fn normalized(self) -> Vec2

// core/src/rng.rs
pub struct Rng { state: u32 }
impl Rng {
    pub fn new(seed: u32) -> Rng;            // seed 0 is mapped to 1
    pub fn next_u32(&mut self) -> u32;       // xorshift32
    pub fn next_f32(&mut self) -> f32;       // in [0, 1)
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32;
    pub fn chance(&mut self, p: f32) -> bool;
}

// core/src/grid.rs
pub struct Grid<T> { w: usize, h: usize, data: Vec<T> }
impl<T: Clone> Grid<T> {
    pub fn new(w: usize, h: usize, fill: T) -> Grid<T>;
    pub fn width(&self) -> usize;
    pub fn height(&self) -> usize;
    pub fn index(&self, x: i32, y: i32) -> Option<usize>;
    pub fn get(&self, x: i32, y: i32) -> Option<&T>;
    pub fn get_mut(&mut self, x: i32, y: i32) -> Option<&mut T>;
    pub fn set(&mut self, x: i32, y: i32, v: T) -> bool;   // false if out of bounds
    pub fn as_slice(&self) -> &[T];
    pub fn as_mut_slice(&mut self) -> &mut [T];
}

// core/src/material.rs
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    Empty, Sand, Water, Oil, Stone, Rock, Wood, Ash, Metal, Glass,
    Lava, Acid, Gunpowder, Ice, Fire, Steam, Smoke, Ember,
}

// core/src/cell.rs
#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub mat: Material,
    pub temp: f32,        // Kelvin-ish scale, ambient = 293.0
    pub fuel: u8,         // remaining burn time for flammable materials
    pub updated: bool,    // tick marker to avoid double moves
}

// core/src/field.rs
pub struct Field { pub w: usize, pub h: usize, pub data: Vec<f32> }
// Air layer is three fields: AirVel (Vec2 grid), pressure, vapor.
pub struct Air {
    pub w: usize,
    pub h: usize,
    pub vel: Vec<Vec2>,      // velocity per cell
    pub pressure: Vec<f32>,
    pub vapor: Vec<f32>,     // humidity, 0..1 per cell
}

// core/src/particle.rs
pub struct Particle {
    pub pos: Vec2,
    pub vel: Vec2,
    pub mat: Material,
    pub temp: f32,
    pub life: f32,           // seconds left, <= 0 means remove
}

// core/src/event.rs
#[derive(Clone, Copy, Debug)]
pub enum SoundKind { Flow, Wind, Crackle, Boom, Hiss, Splash }
#[derive(Clone, Copy, Debug)]
pub enum Event {
    Explosion { x: f32, y: f32, radius: f32, energy: f32 },
    Ignite { x: i32, y: i32 },
    Sound { kind: SoundKind, intensity: f32, x: f32, y: f32 },
}

// core/src/world.rs
pub struct World {
    pub cells: Grid<Cell>,
    pub air: Air,
    pub particles: Vec<Particle>,
    pub events: Vec<Event>,      // drained by app each tick
    pub waves: Vec<f32>,         // ocean height per column, length = width
    pub light: Vec<f32>,         // per-cell light 0..1, filled by systems::lighting
    pub time: f32,               // seconds of sim time
}
impl World { pub fn new(w: usize, h: usize) -> World; }

// systems/src/lib.rs
pub trait System {
    fn name(&self) -> &'static str;
    fn step(&mut self, w: &mut World, rng: &mut Rng, dt: f32);
}
pub fn registry() -> Vec<Box<dyn System>>;   // fixed order, see Task 4

// core/src/time.rs
pub const DT: f32 = 1.0 / 60.0;
pub struct FixedStep { acc: f32 }
impl FixedStep {
    pub fn new() -> FixedStep;
    pub fn push_frame(&mut self, frame_dt: f32) -> u32;   // number of ticks to run this frame
}
```

Shared colour rule: every material has a base colour in `render::palette` (Task 3). Per-cell colour variation is `base + noise(x, y, seed) * 0.06`, so the same world always draws the same.

---

## Dependency Waves

Tasks run in these waves. Within a wave, tasks run as parallel subagents, each in its own git worktree on its own branch. Merge each branch into `sandbox-main` in the listed order before the next wave starts.

- **Wave 0:** Task 1 (serial).
- **Wave 1:** Task 2, then Task 3 and Task 4 in parallel (Task 3 only needs `Grid`, Task 4 only needs `World`).
- **Wave 2:** Tasks 5 through 14 in parallel. Each touches only its own file under `systems/src/` and its own tests, plus one line in the registry (pre-written in Task 4).
- **Wave 3:** Tasks 15, 16 in parallel. Then Task 17 and 18 in parallel.
- **Wave 4:** Task 19 (serial), then Task 20 (serial, integration).

Merge conflicts are expected only in `systems/src/lib.rs` registry lines and `Cargo.toml` dependency lists. Resolve by keeping both sides in the registry's fixed order.

---

### Task 1: Workspace and window scaffold

**Files:**
- Create: `sandbox/Cargo.toml` (workspace), `sandbox/core/Cargo.toml`, `sandbox/core/src/lib.rs`, `sandbox/systems/Cargo.toml`, `sandbox/systems/src/lib.rs`, `sandbox/render/Cargo.toml`, `sandbox/render/src/lib.rs`, `sandbox/audio/Cargo.toml`, `sandbox/audio/src/lib.rs`, `sandbox/app/Cargo.toml`, `sandbox/app/src/main.rs`
- Test: `sandbox/app/tests/smoke.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: empty crates `core`, `systems`, `render`, `audio`, `app` that compile together. `app` depends on the other four.

- [ ] **Step 1: Write the failing smoke test**

`app/tests/smoke.rs` asserts that `sandbox_core::VERSION` is a non-empty string. Add `pub const VERSION: &str = env!("CARGO_PKG_VERSION");` to `core/src/lib.rs` only after the test fails.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p app --test smoke`
Expected: FAIL, package `sandbox_core` has no item `VERSION`.

- [ ] **Step 3: Create the workspace and crates**

Workspace members: `core`, `systems`, `render`, `audio`, `app`. Each library `Cargo.toml` depends only on the crates it needs (`systems` depends on `core`; `render` depends on `core`; `audio` depends on `core`; `app` depends on all four plus `macroquad`). Add `[profile.release] debug = "line-tables-only"` so flamegraphs work later.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p app --test smoke`
Expected: PASS.

- [ ] **Step 5: Open a window**

`app/src/main.rs` opens a macroquad window titled "Sandbox", clears to dark blue, and exits on Escape. Run: `cargo run -p app`. Expected: window opens and closes cleanly on Escape.

- [ ] **Step 6: Record pinned versions and commit**

Copy the macroquad version from `Cargo.lock` into the workspace `Cargo.toml` as an exact pin (`=x.y.z`).

```bash
git add sandbox
git commit -m "chore: scaffold sandbox workspace and window"
```

---

### Task 2: Core data layer

**Files:**
- Create: `core/src/vec2.rs`, `core/src/rng.rs`, `core/src/grid.rs`, `core/src/material.rs`, `core/src/cell.rs`, `core/src/field.rs`, `core/src/particle.rs`, `core/src/event.rs`, `core/src/world.rs`, `core/src/time.rs`
- Modify: `core/src/lib.rs` (re-export all modules)
- Test: `core/tests/grid.rs`, `core/tests/rng.rs`, `core/tests/world.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: every item in the "Shared Interfaces" block above, exactly as written there.

- [ ] **Step 1: Write failing tests**

- `grid.rs`: `Grid::new(4, 3, 0u8)` has width 4 and height 3. `set(3, 2, 9)` returns true and `get(3, 2) == Some(&9)`. `set(4, 0, 1)` returns false. `get(-1, 0)` is `None`. `index(x, y)` round-trips through `as_slice()[idx] == value` for every cell.
- `rng.rs`: `Rng::new(7)` and `Rng::new(7)` produce the same first 1000 `next_u32` values. `Rng::new(0)` does not produce all zeros. `next_f32()` is in `[0, 1)` for 10_000 draws. `chance(0.0)` is never true, `chance(1.0)` is always true.
- `world.rs`: `World::new(64, 48)` has `cells.width() == 64`, `air.vel.len() == 64*48`, `waves.len() == 64`, `light.len() == 64*48`, and every cell is `Material::Empty` at `temp == 293.0`.
- `time.rs`: `FixedStep::push_frame(1.0/60.0)` returns 1. `push_frame(1.0/30.0)` returns 2. `push_frame(0.0)` returns 0 for three consecutive calls and returns 1 on the fourth call after adding `1/60 * 4` total.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p core`
Expected: FAIL, unresolved items.

- [ ] **Step 3: Implement the interfaces**

Implement each type with the exact signatures in "Shared Interfaces". `FixedStep` caps ticks per frame at 8 to avoid a spiral of death; document the cap in a doc comment.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add core
git commit -m "feat(core): add grid, rng, world layers and fixed timestep"
```

---

### Task 3: Palette and pixel buffer rendering

**Files:**
- Create: `render/src/palette.rs`, `render/src/buffer.rs`
- Modify: `render/src/lib.rs`
- Test: `render/tests/palette.rs`, `render/tests/buffer.rs`

**Interfaces:**
- Consumes: `core::{Grid, Cell, Material, World}`, `core::Rng`.
- Produces: `pub fn base_color(m: Material) -> [u8; 4]`, `pub fn cell_color(m: Material, x: i32, y: i32, seed: u32) -> [u8; 4]`, `pub struct FrameBuffer { w, h, pixels: Vec<u8> }` with `pub fn draw_world(&mut self, world: &World, seed: u32)`.

- [ ] **Step 1: Write failing tests**

- `palette.rs`: `base_color(Material::Sand)` is `[194, 178, 128, 255]`. `base_color(Material::Water)` has blue greater than red. `base_color(Material::Empty)` has alpha 0.
- `cell_color(Material::Sand, 3, 5, 42)` equals itself when called twice. Across a 16×16 window the colours are not all identical (variation exists) and every channel differs from base by at most 15.
- `buffer.rs`: a 4×4 world with one `Material::Stone` at (1, 2) writes the stone colour to pixel bytes at index `(2 * 4 + 1) * 4`, and writes alpha 0 for empty cells.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p render --test palette --test buffer`
Expected: FAIL.

- [ ] **Step 3: Implement palette and buffer**

Noise is a hash of `(x, y, seed)` using `wrapping_mul` and xor-shift, mapped to `-1..1`. Base colour table is a `const` array indexed by `material as usize`. `draw_world` writes only the pixel bytes, no allocation per frame (buffer is reused).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p render`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add render
git commit -m "feat(render): palette with per-cell noise and reusable pixel buffer"
```

---

### Task 4: Engine: system trait, registry, events, app loop

**Files:**
- Create: `systems/src/engine.rs`, `systems/src/stubs.rs`
- Modify: `systems/src/lib.rs`, `app/src/main.rs`
- Test: `systems/tests/engine.rs`, `app/tests/loop.rs`

**Interfaces:**
- Consumes: `core::{World, Rng, FixedStep, DT, Event}`.
- Produces: `trait System`, `pub fn registry() -> Vec<Box<dyn System>>`, `pub fn run_tick(world: &mut World, systems: &mut [Box<dyn System>], rng: &mut Rng)`.

- [ ] **Step 1: Write failing tests**

- `engine.rs`: a counting test system increments once per `run_tick`. Two systems registered run in the order registered (record names in a shared `Vec`).
- `stubs.rs`: `registry()` returns exactly these names in this order: `"granular"`, `"liquid"`, `"air"`, `"thermal"`, `"humidity"`, `"fire"`, `"volcano"`, `"particles"`, `"collisions"`, `"explosions"`, `"chemistry"`, `"ocean"`, `"lighting"`. Each stub's `step` does nothing and each returns its name from `name()`.
- `app/tests/loop.rs`: a headless run of 600 ticks on a 64×48 world with empty systems leaves `world.time == 10.0` within `1e-3`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p systems -p app --test loop`
Expected: FAIL.

- [ ] **Step 3: Implement engine and stubs**

`run_tick` advances `world.time` by `DT`, clears `updated` flags on all cells, runs systems in order, and does not drain `world.events` (the app does that after the tick). Stubs are one line each. Each later task replaces one stub with a real module in its own file and keeps the same name string.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p systems -p app`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add systems app
git commit -m "feat(systems): engine loop, registry with stubs for every system"
```

---

### Task 5: Granular and liquid rules (sand, water, oil, acid)

**Files:**
- Create: `systems/src/granular.rs`, `systems/src/liquid.rs`
- Modify: `systems/src/lib.rs` (replace stubs `"granular"` and `"liquid"` with these modules), `systems/src/stubs.rs`
- Test: `systems/tests/granular.rs`, `systems/tests/liquid.rs`

**Interfaces:**
- Consumes: `System`, `World`, `Rng`, `Material`, `Cell`.
- Produces: `pub struct Granular;` and `pub struct Liquid;` both implementing `System` with names `"granular"` and `"liquid"`.

- [ ] **Step 1: Write failing tests**

- Sand column of 5 at x=10, y=0 on empty 16×16 grid: after 60 ticks, all five sand cells sit in row 15 or on top of each other with lowest row 15 (no sand in row 0).
- Sand pile: 40 sand dropped at x=8 settles so that row 15 has at least 3 sand cells and no sand cell is more than 1 row away from a sand or floor cell below its diagonal neighbours.
- Water: 10 water at x=4..6 y=0 on floor row 15 spreads: after 120 ticks, water spans at least 8 columns on row 15.
- Sand sinks through water: a sand cell at (5,5) above a water column reaches row 15 within 60 ticks; the water above moves up by one cell.
- Mass: total sand + water count is unchanged after 500 ticks in a random mixed fill (seed 1).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p systems --test granular --test liquid`
Expected: FAIL.

- [ ] **Step 3: Implement**

Scan rows bottom-up. Use `Cell::updated` to prevent double moves. Granular: fall down, else diagonal down-left/down-right chosen by `rng.chance(0.5)`. Liquid: fall, else diagonal, else sideways with a per-tick flow rule. Density order decides swaps: Sand above Water above Oil. Acid is a liquid with density between Water and Oil and is handled by `chemistry` later (Task 12), so Liquid only moves it.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p systems`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add systems
git commit -m "feat(systems): granular sand and liquid flow with density swaps"
```

---

### Task 6: Air: advection, pressure, buoyancy

**Files:**
- Create: `systems/src/air.rs`
- Modify: `systems/src/lib.rs` (replace stub `"air"`)
- Test: `systems/tests/air.rs`

**Interfaces:**
- Consumes: `World.air`, `World.cells`, `Vec2`, `Rng`, `Material`.
- Produces: `pub struct AirSystem { pub wind: Vec2 }` implementing `System` with name `"air"`. `wind` is the background wind applied every tick.

- [ ] **Step 1: Write failing tests**

- Stable advection: a smoke-marked vapor blob at (8,8) with wind `(2.0, 0.0)` for 30 ticks has its centre of mass moved right by 20 ± 2 cells and no vapor value is `NaN` or negative.
- Pressure equalises: a single high-pressure cell at (4,4) value 10.0 reaches a max deviation below 0.5 from the mean within 200 ticks on an 16×16 grid, with total pressure within `1e-3` relative.
- Buoyancy: a hot cell (temp 600.0) above ambient gains upward velocity `vel.y < 0`.
- Wind alone: with no cells, `wind = (1, 0)` produces `vel.x` close to 1.0 everywhere after 60 ticks.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p systems --test air`
Expected: FAIL.

- [ ] **Step 3: Implement**

Semi-Lagrangian advection (Stam, "Stable Fluids"): for each cell, trace back along `vel` and bilinear-sample the previous field. Pressure: Gauss-Seidel iterations (`const PRESSURE_ITERS: usize = 20`), then subtract the gradient from velocity. Buoyancy: add `(0, -k * (temp - 293.0))` to velocity. Clamp velocity magnitude to `const MAX_VEL: f32 = 4.0`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p systems --test air`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add systems
git commit -m "feat(systems): air advection, pressure solve and buoyancy"
```

---

### Task 7: Thermal: heat diffusion and phase changes

**Files:**
- Create: `systems/src/thermal.rs`
- Modify: `systems/src/lib.rs` (replace stub `"thermal"`)
- Test: `systems/tests/thermal.rs`

**Interfaces:**
- Consumes: `World.cells`, `Cell.temp`, `Material`.
- Produces: `pub struct Thermal;` implementing `System` with name `"thermal"`.

- [ ] **Step 1: Write failing tests**

- Diffusion: one stone cell at 1000.0 on a 9×9 stone grid; after 100 ticks the spread of temperatures (max minus min) is below 200.0 and total heat (sum of temp minus ambient) changes by less than 1e-2 relative.
- Melt: sand at temp 1700.0 becomes `Glass`. Sand at 1000.0 stays `Sand`.
- Boil: water at 400.0 becomes `Steam`. Water at 290.0 stays `Water`.
- Freeze: water at 250.0 becomes `Ice`.
- Ambient: a grid at 293.0 with no sources stays at 293.0 ± 1e-3 for 200 ticks.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p systems --test thermal`
Expected: FAIL.

- [ ] **Step 3: Implement**

Explicit diffusion with `const CONDUCT: f32 = 0.2` and a stability check (`CONDUCT <= 0.25` for 4-neighbour). Phase change table as a `const` array of `(from, to, threshold_up_or_down)`. Heat from `Lava` and `Fire` is applied by their own systems (Tasks 9, 11), not here.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p systems --test thermal`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add systems
git commit -m "feat(systems): heat diffusion with melt, boil and freeze"
```

---

### Task 8: Humidity, steam and rain

**Files:**
- Create: `systems/src/humidity.rs`
- Modify: `systems/src/lib.rs` (replace stub `"humidity"`)
- Test: `systems/tests/humidity.rs`

**Interfaces:**
- Consumes: `World.air.vapor`, `World.cells`, `Material::{Steam, Water, Ice}`.
- Produces: `pub struct Humidity;` implementing `System` with name `"humidity"`.

- [ ] **Step 1: Write failing tests**

- Steam becomes vapor: a Steam cell at (4,4) adds `1.0` to `air.vapor` at that cell and removes the Steam cell in the same tick. Total (vapor + steam count) unchanged.
- Condensation: vapor 0.9 in a cell at temp 280.0 (below dew point 285.0) condenses into one `Water` cell above the floor when that cell is empty; total mass unchanged.
- Rain: a `Water` cell hanging in air with an empty cell below falls by one row per tick (reuse Liquid rules), no new mass.
- Vapor never goes negative and never above `1.0` after 500 ticks of random humid air.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p systems --test humidity`
Expected: FAIL.

- [ ] **Step 3: Implement**

Evaporation: water above `EVAP_TEMP = 350.0` emits vapor, Steam cell rises via air buoyancy from Task 6. Condensation: vapor above `DEW_POINT = 285.0` (cooler cell) converts to Water only in an empty cell and only if vapor exceeds `0.8`. Clamp vapor to `[0, 1]` after each update.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p systems --test humidity`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add systems
git commit -m "feat(systems): humidity, steam, condensation and rain"
```

---

### Task 9: Fire, forests and wet-resistant burning

**Files:**
- Create: `systems/src/fire.rs`
- Modify: `systems/src/lib.rs` (replace stub `"fire"`), `systems/src/stubs.rs` (none)
- Test: `systems/tests/fire.rs`

**Interfaces:**
- Consumes: `World.cells`, `Cell.fuel`, `Cell.temp`, `World.air.vel`, `Event::Ignite`, `Rng`.
- Produces: `pub struct Fire;` implementing `System` with name `"fire"`. Consumes `Event::Ignite` from `world.events` at the start of its step.

- [ ] **Step 1: Write failing tests**

- Ignition: a Wood cell hit by `Ignite` becomes `Fire`, and within 300 ticks becomes `Ash` (burn time set by `fuel`).
- Spread: a 10-wide wood strip with one ignition burns at least 8 of its 10 cells within 600 ticks.
- Wet resistance: a Wood cell with a Water neighbour on each side does not ignite from an adjacent Fire cell in 200 ticks.
- Wind-driven spread: with `air.vel` = `(3, 0)` across the strip, fire reaches the far end at least 30 % faster than with zero wind (measure ticks to first Ash on far cell).
- Fire never persists: no Fire cell remains after 400 ticks without fuel.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p systems --test fire`
Expected: FAIL.

- [ ] **Step 3: Implement**

Fire cell decrements its own `fuel` each tick, emits heat `+40.0` to neighbours, and ignites flammable neighbours (Wood, Gunpowder) with chance `IGNITE_P = 0.15 * (1 + wind_dot)`, where `wind_dot` is the dot product of the neighbour direction and normalised local wind, clamped to `[-0.5, 1.0]`. Wood neighbours with `temp` plus adjacent Water count are less likely (divide chance by `1 + wet_neighbours`). When fuel reaches zero, Fire becomes Ash. Ember and Smoke output are produced by this system too (Smoke rises via air).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p systems --test fire`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add systems
git commit -m "feat(systems): fire spread, wind influence and wet resistance"
```

---

### Task 10: Volcano sources

**Files:**
- Create: `systems/src/volcano.rs`
- Modify: `systems/src/lib.rs` (replace stub `"volcano"`)
- Test: `systems/tests/volcano.rs`

**Interfaces:**
- Consumes: `World.cells`, `Rng`, `Material::{Lava, Rock}`.
- Produces: `pub struct Volcano { pub sources: Vec<VolcanoSource> }` implementing `System` with name `"volcano"`, and `pub struct VolcanoSource { pub x: i32, pub y: i32, pub rate: f32 }`.

- [ ] **Step 1: Write failing tests**

- Emission: one source with `rate = 1.0` (one lava per tick) produces 60 ± 2 Lava cells in 60 ticks on an empty grid.
- Viscosity: Lava spreads sideways at most one cell per 4 ticks (track max x-extent after 40 ticks: at most 10 cells from the source).
- Cooling: Lava cell at temp 800.0 with no heat source turns to `Rock` within 300 ticks.
- Source on edge: a source at x = -1 or x = width emits nothing and does not panic.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p systems --test volcano`
Expected: FAIL.

- [ ] **Step 3: Implement**

Each tick a source accumulates `rate` in `f32` and emits one Lava cell per whole unit into the cell above it if empty. Lava is handled by Liquid rules with a slower sideways move (every 4th tick). Lava's `temp` is set to `1500.0` at emission. Cooling is done by Thermal (Task 7) using its phase table; this task adds the Lava-to-Rock entry to that table.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p systems --test volcano`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add systems
git commit -m "feat(systems): volcano sources with viscous lava and cooling to rock"
```

---

### Task 11: Particles, gravity, collisions

**Files:**
- Create: `systems/src/particles.rs`, `systems/src/collisions.rs`
- Modify: `systems/src/lib.rs` (replace stubs `"particles"` and `"collisions"`)
- Test: `systems/tests/particles.rs`, `systems/tests/collisions.rs`

**Interfaces:**
- Consumes: `World.particles`, `World.cells`, `Vec2`.
- Produces: `pub struct Particles;` (name `"particles"`) integrates gravity and motion. `pub struct Collisions;` (name `"collisions"`) resolves particle vs grid and particle vs particle.

- [ ] **Step 1: Write failing tests**

- Projectile range: a particle launched at 45° with speed 20 m/s on flat ground (no drag) lands within 1 % of `v² / g` (g = 9.81) when measured with fixed `DT`.
- Gravity: a dropped particle's velocity after 60 ticks is `9.81` ± 0.05 downward.
- Life: a particle with `life = 0.5` is removed after 31 ticks.
- Bounce: a particle hitting a Stone cell reverses vertical velocity with factor `0.6` (restitution), speed ratio within 1e-3.
- Particle-particle: two particles on a collision course exchange momentum; total momentum (mass 1.0 each) unchanged within 1e-4.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p systems --test particles --test collisions`
Expected: FAIL.

- [ ] **Step 3: Implement**

Particles: semi-implicit Euler with `GRAVITY = Vec2 { x: 0.0, y: 9.81 }` in cells per second squared (scale fixed in one constant). Collisions: sample the cell at the particle's new position; if solid, reflect and apply restitution. Particle-particle is an O(n²) check for n ≤ 512, else a grid bucket (Task 17 may replace it; the test must still pass). Remove particles with `life <= 0` or outside the grid.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p systems`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add systems
git commit -m "feat(systems): particle gravity, grid bounce and particle collisions"
```

---

### Task 12: Explosions

**Files:**
- Create: `systems/src/explosions.rs`
- Modify: `systems/src/lib.rs` (replace stub `"explosions"`)
- Test: `systems/tests/explosions.rs`

**Interfaces:**
- Consumes: `Event::Explosion` from `world.events`, `World.air.pressure`, `World.cells`, `World.particles`, `Rng`.
- Produces: `pub struct Explosions;` implementing `System` with name `"explosions"`.

- [ ] **Step 1: Write failing tests**

- Falloff: pressure added at distance d from centre is monotonically non-increasing in d for d in 0..=radius.
- Destruction: sand and stone inside radius 3 are removed (become Empty or debris particles); stone at distance greater than radius is untouched.
- Debris: an explosion with energy 50.0 spawns between 5 and 50 particles with speed greater than zero.
- Scorch: wood inside radius becomes `Ash`, cells get `temp > 1000.0`.
- Edge: an explosion at (0, 0) and at (width-1, height-1) does not write out of bounds and does not panic.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p systems --test explosions`
Expected: FAIL.

- [ ] **Step 3: Implement**

Process `Event::Explosion` by walking the square `[x-r, x+r] x [y-r, y+r]` clipped to the grid. Pressure added = `energy * (1 - d/r)²`. Cells with `Material::{Sand, Stone, Wood, Metal}` inside radius `r * 0.5` become debris particles; others within `r` become Empty or Ash. Debris velocity points radially outward with random magnitude from `rng.range_f32`. Shockwave ring: a thin band of pressure at radius `r * (t / T)` for the next `T = 0.3` seconds is added in later ticks by storing active shocks in a `Vec` on the system (`pub shocks: Vec<Shock>`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p systems --test explosions`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add systems
git commit -m "feat(systems): explosions with pressure falloff, debris and scorch"
```

---

### Task 13: Chemistry

**Files:**
- Create: `systems/src/chemistry.rs`
- Modify: `systems/src/lib.rs` (replace stub `"chemistry"`)
- Test: `systems/tests/chemistry.rs`

**Interfaces:**
- Consumes: `World.cells`, `World.events`, `Material`.
- Produces: `pub struct Chemistry;` implementing `System` with name `"chemistry"`, and `pub const REACTIONS: &[Reaction]` where `pub struct Reaction { pub a: Material, pub b: Material, pub out_a: Material, pub out_b: Material, pub chance: f32, pub emit: Option<Event-kind> }` (emit is a plain `ReactionEmit` enum, `None | Ignite | Steam` to avoid circular types).

- [ ] **Step 1: Write failing tests**

- Acid + Metal: an Acid cell adjacent to a Metal cell with chance 1.0 becomes `Empty`/`Steam` and the Metal becomes `Empty`; a `Steam` cell appears; no mass gained.
- Acid + Stone: slow reaction, chance 0.02, over 100 ticks removes between 0 and 3 stone cells.
- Gunpowder chain: a line of 20 Gunpowder with one Fire at the start is fully Ash or Fire within 200 ticks; each Gunpowder cell becomes Fire via `ReactionEmit::Ignite`.
- Table integrity: every `Reaction` has `chance` in `0.0..=1.0`, and no two entries share the same `(a, b)` pair.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p systems --test chemistry`
Expected: FAIL.

- [ ] **Step 3: Implement**

`REACTIONS` is a `const` table. Each tick, for every cell check its four neighbours against the table (both orderings). On match, roll `rng.chance(chance)`, write outputs, and for `ReactionEmit::Ignite` push `Event::Ignite` into `world.events`. Each cell can react at most once per tick (use `updated`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p systems --test chemistry`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add systems
git commit -m "feat(systems): data-driven chemistry table with chain reactions"
```

---

### Task 14: Ocean waves and water surface

**Files:**
- Create: `systems/src/ocean.rs`
- Modify: `systems/src/lib.rs` (replace stub `"ocean"`)
- Test: `systems/tests/ocean.rs`

**Interfaces:**
- Consumes: `World.waves`, `World.time`, `World.cells` (for water level), `Rng`.
- Produces: `pub struct Ocean { pub wind_speed: f32, pub sea_level: i32 }` implementing `System` with name `"ocean"`. Writes `world.waves` each tick and nudges Water cells above `sea_level` down into the wave shape.

- [ ] **Step 1: Write failing tests**

- Calm: with `wind_speed = 0.0`, every value in `world.waves` is `0.0` ± 1e-6 after any tick count.
- Bounded: with `wind_speed = 5.0`, every wave value stays within `[-AMP_MAX, AMP_MAX]` where `AMP_MAX = 6.0` cells, for 2000 ticks.
- Moves: with wind, the crest position changes over time (crest index at tick 100 differs from tick 0).
- Periodic: the wave array length equals `world.cells.width()`.
- No NaN: 10_000 ticks produce no `NaN` in `world.waves`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p systems --test ocean`
Expected: FAIL.

- [ ] **Step 3: Implement**

Sum of three sines per column: `A_i * sin(k_i * x - w_i * t)` with three fixed `(A, k, w)` tuples in a `const` array, amplitudes scaled by `wind_speed / 5.0` clamped to `[0, 1]`. Water surface: Water cells above `sea_level + wave` are moved to the level, displaced water goes into the nearest air column (mass conserved).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p systems --test ocean`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add systems
git commit -m "feat(systems): wind-driven ocean waves on a height field"
```

---

### Task 15: Lighting and motion smoothing

**Files:**
- Create: `systems/src/lighting.rs`, `render/src/smooth.rs`
- Modify: `systems/src/lib.rs` (replace stub `"lighting"`), `render/src/lib.rs`
- Test: `systems/tests/lighting.rs`, `render/tests/smooth.rs`

**Interfaces:**
- Consumes: `World.cells`, `World.light`, `Material::{Lava, Fire, Ember}`, `Cell.temp`.
- Produces: `pub struct Lighting;` (name `"lighting"`) fills `world.light` each tick. `pub fn lerp(a: f32, b: f32, t: f32) -> f32` and `pub fn interp_world_alpha(prev: &[f32], cur: &[f32], alpha: f32, out: &mut [f32])` in `render::smooth`.

- [ ] **Step 1: Write failing tests**

- Emitter: one Lava cell on a dark grid gives light `1.0` at its cell; light at distance 4 is less than light at distance 1; monotone in distance.
- Shadow: a Stone wall between a Lava cell and a probe cell blocks at least 80 % of the light compared with no wall.
- Range: every light value is in `[0, 1]`.
- Smooth: `lerp(a, b, 0.0) == a`, `lerp(a, b, 1.0) == b`; `interp_world_alpha` at alpha 0.5 equals the midpoint within 1e-6.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p systems --test lighting -p render --test smooth`
Expected: FAIL.

- [ ] **Step 3: Implement**

Lighting: each emitter (Lava, Fire, Ember) casts rays to cells in radius 12, attenuating by `1 / (1 + d²)`, blocked by solid cells (Stone, Rock, Metal, Glass). Recomputed every tick on the emitter set only. Smoothing: render interpolates between previous and current world snapshots with `alpha = accumulator / DT` from `FixedStep`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p systems --test lighting -p render --test smooth`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add systems render
git commit -m "feat: light from emitters with shadows and render-side smoothing"
```

---

### Task 16: Config and save/load

**Files:**
- Create: `core/src/config.rs`, `core/src/save.rs`, `config/default.toml`
- Modify: `core/src/lib.rs`, `core/Cargo.toml` (add `serde`, `toml`)
- Test: `core/tests/config.rs`, `core/tests/save.rs`

**Interfaces:**
- Consumes: `World`, `Material`, `Cell`, `Air`.
- Produces: `pub struct Config { pub seed: u32, pub wind: f32, pub sea_level: i32, pub brush: u32, pub volume: f32 }` with `Default`, `pub fn load_config(path: &Path) -> (Config, Option<String>)` (second value is the warning message), `pub fn save_world(w: &World, path: &Path) -> std::io::Result<()>`, `pub fn load_world(path: &Path) -> Result<World, SaveError>`, and `pub enum SaveError { Io(std::io::Error), BadVersion(u32), Corrupt(String) }` implementing `Display`.

- [ ] **Step 1: Write failing tests**

- Config: a missing file returns `Config::default()` and `Some(warning)`. A malformed TOML returns defaults and `Some(warning)`. A valid file with `wind = 1.5` returns `wind == 1.5` and `None`.
- Save: `save_world` then `load_world` returns a world whose `cells` equal the original cell by cell and whose `air.vel` equals the original.
- Version: a file whose first line is `version 99` returns `Err(SaveError::BadVersion(99))`.
- Corrupt: a file truncated halfway returns `Err(SaveError::Corrupt(_))`, does not panic.
- Atomicity: a failed save leaves the existing file unchanged (write to `path.tmp`, then rename).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p core --test config --test save`
Expected: FAIL.

- [ ] **Step 3: Implement**

Save format: first line `version 1`, then one line per cell `mat temp fuel`, then one line per air vel and pressure and vapor. Plain text, no binary, so it is inspectable. Config file lives at `config/default.toml` and is read from the working directory; the path is passed in, not hardcoded.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add core config
git commit -m "feat(core): toml config with fallback and versioned save/load"
```

---

### Task 17: Audio

**Files:**
- Create: `audio/src/lib.rs` (replace stub), `audio/src/mapping.rs`, `audio/src/player.rs`
- Modify: `audio/Cargo.toml` (add `rodio`)
- Test: `audio/tests/mapping.rs`, `audio/tests/nodevice.rs`

**Interfaces:**
- Consumes: `core::{Event, SoundKind}`.
- Produces: `pub struct AudioHandle { tx: std::sync::mpsc::Sender<AudioCmd> }` with `pub fn start() -> AudioHandle` (returns a handle with a dead channel if no device), `pub fn send_events(&self, events: &[Event])`. `pub fn map_event(e: &Event) -> Option<AudioCmd>` is pure and tested.

- [ ] **Step 1: Write failing tests**

- Mapping: `Event::Explosion { energy: 50.0, .. }` maps to `AudioCmd::Play { kind: Boom, gain }` with `gain` in `(0, 1]`, and larger energy gives larger gain.
- Mapping: `Event::Ignite` maps to `Crackle` with gain `0.2`.
- `Event::Sound { kind: Flow, intensity: 0.0, .. }` maps to `None` (silence costs nothing).
- No device: `start()` in a test environment with no output device returns a handle; `send_events` with 1000 events does not panic and does not block (completes within 100 ms).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p audio`
Expected: FAIL.

- [ ] **Step 3: Implement**

Audio thread owns the `rodio` output stream. It receives `AudioCmd` over `mpsc`, plays short synthesised buffers (noise bursts for flow and hiss, sine-decayed for boom, short random ticks for crackle) and drops commands when the queue exceeds 64 entries. The sim never waits on audio.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p audio`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add audio
git commit -m "feat(audio): event-to-sound mapping and non-blocking audio thread"
```

---

### Task 18: Performance: dirty chunks, parallel bands, memory reuse

**Files:**
- Create: `core/src/chunk.rs`, `systems/src/parallel.rs`
- Modify: `core/src/lib.rs`, `systems/src/granular.rs`, `systems/src/liquid.rs` (use chunk skipping), `systems/Cargo.toml` (add `rayon`), `core/Cargo.toml`
- Test: `core/tests/chunk.rs`, `systems/tests/parallel.rs`
- Bench: `systems/benches/tick.rs` (criterion)

**Interfaces:**
- Consumes: `World`, `Grid`.
- Produces: `pub struct Chunks { pub size: usize, pub active: Vec<bool>, pub w: usize, pub h: usize }` in `core::chunk` with `mark(x, y)`, `is_active(cx, cy)`, `clear()`. `pub fn tick_parallel(world: &mut World, ...)` in `systems::parallel` that splits rows into bands and runs the granular and liquid rules per band with `split_at_mut`.

- [ ] **Step 1: Write failing tests**

- Chunks: marking cell (17, 3) with size 16 activates chunk (1, 0) only. `clear()` deactivates all.
- Skip correctness: with dirty chunks on, the world after 300 ticks equals the world with dirty chunks off (byte-identical cell output, same seed).
- Parallel determinism: `tick_parallel` with 1 thread and with 8 threads produce identical worlds after 300 ticks from seed 5.
- Memory: running 1000 ticks performs zero heap allocations per tick after warm-up (checked with a counting global allocator in a test binary).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p core --test chunk -p systems --test parallel`
Expected: FAIL.

- [ ] **Step 3: Implement**

Chunk size 16. A chunk is active if any cell in it or its neighbour changed last tick. Bands are horizontal strips of at least 2 chunk rows; each band is processed by one rayon task with `split_at_mut` on the cell slice, so no unsafe code is needed. Bench: `cargo bench -p systems` reports the tick time for the default scene. Record the number in the plan's commit message.

- [ ] **Step 4: Run tests and benchmark**

Run: `cargo test -p core -p systems`
Run: `cargo bench -p systems`
Expected: tests PASS. Bench reports a tick time; it must be under the Global Constraints budget (8 ms) on the development Mac in release mode, or the task is not done and the gap is reported.

- [ ] **Step 5: Commit**

```bash
git add core systems
git commit -m "perf: dirty chunks, parallel row bands, zero-alloc steady state"
```

---

### Task 19: GPU render path behind feature flag

**Files:**
- Create: `render/src/gpu.rs`, `render/src/shader.wgsl`
- Modify: `render/Cargo.toml` (feature `gpu`, optional `wgpu` and `bytemuck`), `render/src/lib.rs`, `app/src/main.rs` (choose backend by feature)
- Test: `render/tests/gpu_parity.rs` (run only with `--features gpu`)

**Interfaces:**
- Consumes: `FrameBuffer` from Task 3.
- Produces: `pub trait Presenter { fn present(&mut self, fb: &FrameBuffer); }` implemented by a macroquad texture presenter (default) and a `pub struct GpuPresenter` (feature `gpu`).

- [ ] **Step 1: Write failing tests**

- Parity: on a golden 128×96 scene, GPU presenter's readback equals the CPU `FrameBuffer` within 1 unit per channel for at least 99.9 % of pixels.
- Default: without `--features gpu`, `app` builds and runs with the macroquad presenter.
- Resize: after a resize event to 0×0, the GPU presenter skips the frame and does not panic.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p render --features gpu --test gpu_parity`
Expected: FAIL.

- [ ] **Step 3: Implement**

Upload the `FrameBuffer` bytes into a texture each frame with `queue.write_texture`. Fullscreen triangle in `shader.wgsl` samples the texture with nearest filtering. Glow from Task 15 can be added as a second pass later (out of scope here). Surface errors (`Lost`, `Outdated`) reconfigure and retry once.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p render --features gpu`
Expected: PASS on the development Mac.

- [ ] **Step 5: Commit**

```bash
git add render app
git commit -m "feat(render): optional wgpu presenter with parity test"
```

---

### Task 20: Cross-platform builds and CI

**Files:**
- Create: `.github/workflows/ci.yml`, `app/src/platform.rs`
- Modify: `app/Cargo.toml` (target-specific dependencies), `app/src/main.rs`
- Test: `app/tests/platform.rs`

**Interfaces:**
- Consumes: `app` entry point.
- Produces: `pub fn config_dir() -> std::path::PathBuf` in `app::platform` (macOS and Linux use `$XDG_CONFIG_HOME` or `~/.config/sandbox`; Windows uses `%APPDATA%\sandbox`; wasm returns `"."` and skips file IO).

- [ ] **Step 1: Write failing tests**

- `config_dir()` returns a path ending in `sandbox` on native targets.
- On `wasm32`, the save/load and config code paths compile without `std::fs` usage (checked by `cargo check --target wasm32-unknown-unknown -p app`).
- CI matrix builds and tests on `macos-latest`, `ubuntu-latest`, `windows-latest` on every push.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p app --test platform`
Expected: FAIL.

- [ ] **Step 3: Implement**

`platform.rs` isolates `cfg(target_os)` and `cfg(target_arch = "wasm32")`. File IO in `app` goes only through `platform`. CI runs `cargo test --workspace` on the three OS targets and `cargo check --target wasm32-unknown-unknown -p app` on Linux. Build the wasm bundle with `macroquad`'s web build instructions and record the command in `README.md`.

- [ ] **Step 4: Run tests and check targets**

Run: `cargo test --workspace`
Run: `rustup target add wasm32-unknown-unknown && cargo check --target wasm32-unknown-unknown -p app`
Expected: PASS on the development Mac, wasm check passes.

- [ ] **Step 5: Commit**

```bash
git add .github app
git commit -m "ci: test matrix across macOS, Linux, Windows and wasm check"
```

---

### Task 21: Integration: the sandbox app

**Files:**
- Create: `app/src/scenes.rs`, `app/src/hud.rs`, `app/src/controls.rs`, `config/scenes/volcano_island.toml`, `config/scenes/forest.toml`, `config/scenes/ocean_storm.toml`, `config/scenes/explosion_lab.toml`
- Modify: `app/src/main.rs` (wire everything), `app/Cargo.toml`
- Test: `app/tests/scenes.rs`, `app/tests/soak.rs`

**Interfaces:**
- Consumes: every crate above.
- Produces: `pub enum Scene { VolcanoIsland, Forest, OceanStorm, ExplosionLab }` with `pub fn build(scene: Scene, seed: u32) -> World` and `pub fn systems_for(scene: Scene) -> Vec<Box<dyn System>>`.

- [ ] **Step 1: Write failing tests**

- Each scene builds a world with at least one non-empty material of its theme (volcano has Lava, forest has Wood, ocean has Water, explosion lab has Gunpowder).
- Soak: each scene runs 10 000 headless ticks with seed 1 and ends with no `NaN` in any float field, no panic, and total cell count of solid materials within `1e-3` drift of the start (except Explosion lab, which is allowed to drop mass from explosions and must record it).
- Determinism: same scene and seed run twice produce identical save output bytes.
- Controls: keys 1–8 select materials, `[` and `]` change brush size, `S` saves, `L` loads, `Space` pauses, `Escape` quits. Test `controls::map_key` as a pure function.
- HUD: `hud::format_line(fps, tick_ms)` returns `"FPS 60  tick 3.2 ms"` for `fps = 60.0`, `tick_ms = 3.2`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p app --test scenes --test soak`
Expected: FAIL.

- [ ] **Step 3: Implement**

`main` loop: pump macroquad events, convert to `controls` actions, `FixedStep::push_frame` gives tick count, run `run_tick` that many times with the system list, drain `world.events` into `audio.send_events`, draw with `render::FrameBuffer` and the chosen presenter, draw HUD. Config loaded once at start with `load_config`; warning shown in HUD if present. Scene files under `config/scenes/` are read the same way as `default.toml`.

- [ ] **Step 4: Run tests and play**

Run: `cargo test --workspace`
Run: `cargo run --release -p app`
Expected: all tests PASS. Window shows the volcano island scene by default. Manual check: lava flows, water sits in the basin, wind moves smoke, and explosions produce sound.

- [ ] **Step 5: Commit**

```bash
git add app config
git commit -m "feat(app): scenes, controls, HUD and full sandbox loop"
```

---

## Final verification

Run the whole suite and the release build from the workspace root:

```bash
cd /Users/anujbarve/Documents/rusty/sandbox
cargo test --workspace
cargo bench -p systems
cargo run --release -p app
```

Expected: all tests pass, bench tick time under 8 ms for the default scene, and the app runs for 60 seconds without a panic.

---

## Self-Review

**1. Spec coverage:** Sand (Task 5), water and oil (Task 5), air and wind (Task 6), pressure (Task 6), steam and humidity and rain (Task 8), heat and temperature (Task 7), melting and boiling (Task 7), fire and forests (Task 9), volcano (Task 10), projectiles and gravity (Task 11), collisions (Task 11), explosions (Task 12), chemistry (Task 13), ocean waves (Task 14), lighting and glow and shadows (Task 15), motion smoothing (Task 15), sound (Task 17), config and save (Task 16), dirty chunks and threads and zero-alloc (Task 18), WGPU (Task 19), cross-platform and CI (Task 20), scenes and HUD and controls (Task 21), fixed timestep (Task 2 and 4). Realism checks (conservation, stability) are in the Global Constraints and in each task's tests. Covered.

**2. Step scan:** Interface block is frozen once in Task 2 and 4; later tasks reference it. Several tasks (13, 14, 17) name types the implementer must still choose details for (e.g. the exact `ReactionEmit` variants are given; the `AudioCmd` variants are named in tests only). Gap: `AudioCmd` is not defined in Interfaces. Fix: it is defined by Task 17's own file, and Task 17 tests reference only `Play { kind, gain }`. Acceptable because only Task 17 consumes it. Gap noted, not blocking.

**3. Type consistency:** `Material` variants used in tests (`Wood`, `Gunpowder`, `Steam`, `Lava`, `Rock`, `Ember`) all appear in the enum. `Event::Ignite` takes `i32` coords and `Event::Explosion` takes `f32`; Task 9 and Task 12 use these consistently. `Ocean` fields match Task 21 usage. `Config` fields match Task 16 and Task 21.

**4. Review Focus:** 
- Resize/minimize: Task 19 (resize test) and Task 1 window loop. Covered.
- Edge placement: Task 10 (source on edge), Task 12 (explosion corners). Covered.
- Config malformed: Task 16 (malformed TOML test). Covered.
- Corrupt save: Task 16 (truncated test, version test). Covered.
- Audio device missing: Task 17 (no-device test). Covered.

Gap: Task 21 soak test covers NaN and mass, but not the "float NaN clamp" warning path. Accepted as covered by the soak assertion.

**5. Proportion:** The plan is long because it spans 21 subsystems. Code blocks are limited to the shared interface contract; per-task steps name tests and signatures, not bodies. Acceptable.

---

## Execution Handoff

Plan saved to `docs/superpowers/plans/2026-11-05-sandbox-full-build.md`. Please review. Choose an execution approach:

- **Subagent-driven**: a fresh subagent implements each task and a fresh reviewer checks it before the next starts, then a whole-branch review at the end. Most thorough; costs a fresh context per task and per review.
- **Native**: I implement every task in this session, then one fresh reviewer checks the whole branch. Cheapest and fastest; no independent review until the end.

For this plan I recommend **Subagent-driven**, because tasks 5 through 14 touch shared `World` and registry code and the waves depend on each other's interfaces, and a shipped mistake in the physics would be hard to see without per-task tests.

Does the plan capture what you want, and which approach should we use?
