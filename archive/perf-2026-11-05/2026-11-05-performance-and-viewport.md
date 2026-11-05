# Performance and viewport implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the GPU sand engine fast enough for 60 fps at 1080p (about 0.83 ms per step, 20 steps per frame), and add a zoomable viewer that spends its compute on the cells the player can see. Every change is measured with the bench before and after, and every change that claims to be exact is checked bit-for-bit against the CPU reference.

**Architecture:** Three tracks. (A) Measure: per-pass GPU timestamps and throwaway spikes that bound each idea before it is built. (B) Dispatch and readback: indirect dispatch of awake chunks, sub-rectangle readback, and a per-step cost reduction in the wake pass. (C) Viewport: a camera (pan and zoom) that drives a visible-chunk mask. The mask first becomes a set of chunks that are not computed at all, which is an approximation at the view edge that is measured and bounded. Only then does it become an optional time-sliced mode, which is also an approximation. Exact physics stays the default and stays bit-identical to the CPU reference.

**Tech Stack:** Rust 2024 crate `sand` in `sand/`; wgpu 30 compute (WGSL, Metal on Apple M5); macroquad for the viewer; rayon for the CPU reference; `cargo test --release`, `cargo run --release --bin bench -- <secs>`.

**Spec:** No separate spec file. The Goal and the Review Focus below are the requirements. Measurements come from `docs/lessons/14-gpu-perf-fused-kernel.md`, `docs/lessons/13-chunk-sleeping.md` and `sand/src/bin/bench.rs` output of 2026-11-05 (M5, busy scene, per step, GPU timestamps): 640×360 0.235 ms, 1080p 2.247 ms, 4K 6.284 ms, copy-only floor at 4K 0.691 ms (lesson 14). Settled 4K: 4.385 ms with sleeping, 4.870 ms without. Readback adds about 2.2 ms at 4K (once per frame).

## Global Constraints

- Any mode that claims to be exact must give a world bit-identical to `CpuSim::step_no_sleep`, for every grid size including odd sizes. Existing tests: `gpu_gives_identical_world_to_cpu`, `gpu_gives_identical_world_to_cpu_at_odd_size`, `sleeping_gives_identical_world_to_no_sleeping`, `gpu_matches_cpu_on_odd_sizes`, `gpu_matches_cpu_with_jets_freezers_metal_and_coal`.
- Chunk side `CHUNK = 32` (`physics/chunks.rs`), mirrored in WGSL. A workgroup is `GROUP_W = GROUP_H = 16` threads of 2×2 cells, so one workgroup covers exactly one chunk when the block offset is 0.
- The Margolus block offset is `frame & 1`. Blocks at odd frames straddle chunk boundaries. Any scheduling change must account for this.
- Max cell speed is one cell per step (`MAX_SPEED = CELL_METERS / STEP_SECONDS`, 5 mm per 1/1200 s). Used to size the view margin.
- Heat and motion stay fused in one dispatch unless the spike in Task 1 shows a split wins (lesson 14: the two-pass version was slower).
- Timestamp queries are optional. Anything new must degrade to "no timing" on adapters without `TIMESTAMP_QUERY`, as `GpuTimer` does now.
- No new crates without asking. Use what is in `Cargo.toml`.
- Every Rust constant that the shader needs is declared in WGSL and read back in Rust by the same string-parse trick as `shader_const` in `physics/gpu.rs`, or both sides are generated from one place. Do not let them drift.
- Paint, probe and audio must agree with what is drawn (same mapping from screen to cell).

## Review Focus

1. **Wrong cell under the cursor after zoom or pan.** Painting or probing at zoom 4× lands in the wrong cell (off by the letterbox offset or by the old `world_rect`). Expected: the cell under the pointer is the cell drawn there. Covered by Task 5.
2. **Frozen chunk comes back stale.** A chunk frozen while off-screen has old values in one ping-pong buffer, so when it is unfrozen the heat pass reads the wrong neighbour and sand jumps or freezes. Expected: a chunk that was frozen and then reactivated behaves as if it never stopped, at the cost of a defined, bounded difference at the view edge. Covered by Task 7.
3. **Sand stops at a chunk border.** With indirect dispatch, a 2×2 block that straddles an awake and an asleep chunk at an odd frame is dropped and its awake cells freeze. Expected: a falling grain crosses every chunk boundary at every frame parity. Covered by Task 3.
4. **Camera leaves the grid.** Zoom or pan can move the view past the grid edge, over the toolbar, or to a zero-size grid. Expected: the camera is clamped; the view never shows blank beyond the grid and never reads cells outside it. Covered by Task 4.
5. **Resize, grid change or reset while zoomed.** Changing grid size, loading a scene or resizing the window keeps a stale visible mask, so chunks that are now visible stay frozen. Expected: a change of grid or scene rebuilds the mask and wakes the newly visible chunks in the same frame. Covered by Task 7.

Each line above has its test in the task that owns the code.

---

## File Structure

Created:
- `sand/src/bin/physics/camera.rs`: pan and zoom state, screen-to-cell and cell-to-screen mapping, clamping, the visible chunk mask.
- `sand/src/physics/schedule.rs`: pure Rust functions that decide which chunks run in a step (dilation of awake flags, the visible set, the time-slice rule). No GPU code, so the rules are unit-testable on the CPU.
- `sand/src/bin/profile.rs`: runs the step per pass with timestamps and prints a per-pass table. Reuses `bench` scene setup.
- `docs/lessons/21-indirect-dispatch-and-the-viewport.md`: lesson write-up (after the code lands, per repo convention).

Modified:
- `sand/src/physics/gpu.rs`: indirect dispatch buffers, per-pass timestamps, sub-rectangle readback, mask upload, forced freeze and its buffer sync.
- `sand/src/physics/wgsl/wake.wgsl`: writes the dispatch list and dispatch arguments, dilated by one chunk right and down.
- `sand/src/physics/wgsl/step.wgsl`: maps `workgroup_id` to a chunk through the list.
- `sand/src/physics/cpu.rs`: the same schedule rules, so CPU and GPU agree.
- `sand/src/physics/tests.rs`: new tests listed per task.
- `sand/src/bin/physics/main.rs`: camera use for input, readback rectangle, mask wiring.
- `sand/src/bin/physics/render.rs`: draws the visible sub-rectangle of the grid texture into the camera's destination rectangle.
- `sand/src/bin/physics/settings.rs`, `sand/src/bin/physics/ui.rs`: the off-screen mode switch and the zoom indicator.
- `sand/src/audio/analysis.rs`: optional, only if Task 2 shows analysis above 1 ms.

---

## Expected gains (what the plan promises, and how each claim is checked)

| Item | Estimated gain | Source of the estimate | Checked by |
|---|---|---|---|
| Per-pass timing | none (tooling) | | Task 2 |
| Heat-off spike upper bound | sets the ceiling for Task 9 | run with heat disabled | Task 1 |
| Indirect dispatch | 0% busy, up to ~10% settled 4K | sleep vs no-sleep gap (4.39 vs 4.87 ms); idle workgroups launch with no work | Task 3 bench |
| Sub-rectangle readback | 4× less readback at 2× zoom-in | readback 2.2 ms at 4K full view | Task 6 bench |
| Viewport compute (zoom in) | roughly proportional to the fraction of the grid visible, plus the margin | cost is per awake chunk | Task 8 bench at 1/4 and 1/16 view |
| Heat every N steps | at most (heat share)×(1−1/N) | heat share unknown | Task 9 after spike |
| Kernel micro-optimisations | bounded by the 0.69 ms copy floor at 4K (about 9×) | lesson 14 | Task 10 after per-pass timing |
| Audio analysis on GPU | only if analysis costs over 1 ms | unmeasured | Task 11 |

Targets: 1080p at 60 fps needs at most 0.83 ms per step (busy scene, currently 2.25 ms). 4K at 60 fps needs at most 0.83 ms per step (currently 6.3 ms), which means the kernel work must succeed; scheduling alone will not reach it.

---

## Task 0: Baseline record

**Files:**
- Create: `docs/plans/perf-baseline-2026-11-05.md` (numbers only)

**Interfaces:**
- Consumes: `cargo run --release --bin bench -- 2` output.
- Produces: the baseline table that every later task compares against.

- [ ] **Step 1: Run the bench three times on a quiet machine**

Run: `cd sand && cargo run --release --bin bench -- 3` (three times, same machine, same power state)
Expected: the tables print; ms numbers vary by under 5% between runs. If not, close other GPU apps and repeat.

- [ ] **Step 2: Write the median of the three runs into `docs/plans/perf-baseline-2026-11-05.md`**

Include the two tables and the machine (`Apple M5 (Metal)`). Do not change code in this task.

- [ ] **Step 3: Commit**

```bash
git add docs/plans/perf-baseline-2026-11-05.md
git commit -m "docs(perf): record bench baseline before optimisation work"
```

---

## Task 1: Upper-bound spikes (throwaway, not committed)

**Files:**
- Modify (temporary, reverted): `sand/src/physics/wgsl/thermal.wgsl`, `sand/src/physics/wgsl/common.wgsl`, `sand/src/physics/wgsl/step.wgsl`

**Interfaces:**
- Consumes: baseline from Task 0.
- Produces: three numbers written into `docs/plans/perf-baseline-2026-11-05.md` under "Spikes": ms per step with heat disabled, with the random hash replaced by a constant, and with `GROUP_W = GROUP_H = 8`.

- [ ] **Step 1: Heat-off spike.** In `step.wgsl`, skip the heat computation (write the old cell as the new cell). Run bench at 1080p and 4K, busy scene. Record ms/step. This is the ceiling for Task 9.

- [ ] **Step 2: RNG-off spike.** Replace the `hash3` call in `common.wgsl` with a constant. Record. This is the ceiling for the RNG part of Task 10.

- [ ] **Step 3: Workgroup spike.** Set `GROUP_W` and `GROUP_H` to 8 in `common.wgsl`. Run the bench. Record. Note that this breaks the one-workgroup-per-chunk mapping of Task 3, so if it wins the mapping must change; if it loses, keep 16.

- [ ] **Step 4: Revert everything**

Run: `git checkout -- sand/src/physics/wgsl/`
Expected: `git status` clean for those paths.

- [ ] **Step 5: Commit the numbers only**

```bash
git add docs/plans/perf-baseline-2026-11-05.md
git commit -m "docs(perf): record upper-bound spikes (heat, RNG, workgroup size)"
```

---

## Task 2: Per-pass GPU timing

**Files:**
- Modify: `sand/src/physics/gpu.rs` (the `step` function and `GpuTimer`)
- Create: `sand/src/bin/profile.rs`
- Test: `sand/src/physics/tests.rs`

**Interfaces:**
- Consumes: `GpuSim::step(&mut self, steps: u32)`, `GpuTimer` as it is.
- Produces: `GpuSim::pass_times_ms(&mut self) -> Option<PassTimes>` where `pub struct PassTimes { pub step_ms: f64, pub wake_ms: f64 }` per step, the latest harvested value. Non-blocking, like `gpu_time_ms`.

Note: Apple GPUs do not support timestamps inside a pass (`gpu.rs` comment). Each dispatch therefore goes in its own compute pass with its own `timestamp_writes`. This costs a little pass overhead; the profile binary reports it separately.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn per_pass_times_are_reported_when_timestamps_exist() {
    let world = World::new(256, 256);
    let Some(mut gpu) = gpu_or_skip(&world) else { return };
    if !gpu.has_timestamps() { return; }
    gpu.step(200);
    gpu.wait();
    let t = wait_for(|| gpu.pass_times_ms()).expect("timestamps arrive");
    assert!(t.step_ms > 0.0 && t.wake_ms >= 0.0);
}
```

(`wait_for` is a small helper that polls `gpu.pass_times_ms()` until `Some` or fails after 5 s. Add it next to `gpu_or_skip`.)

- [ ] **Step 2: Run to verify it fails**

Run: `cd sand && cargo test --release per_pass_times -- --nocapture`
Expected: FAIL, `pass_times_ms` not found.

- [ ] **Step 3: Implement.** In `GpuSim::step`, record `timestamp_writes` on each pass (step and wake) into separate timer slots; `GpuTimer` gets a second query pair per slot. Add `pass_times_ms` mirroring `gpu_time_ms`.

- [ ] **Step 4: Run to verify it passes**

Run: `cd sand && cargo test --release per_pass_times -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Add `profile` binary** that prints, per grid size and scene, `step_ms`, `wake_ms`, and the sum. Run it: `cargo run --release --bin profile`.

- [ ] **Step 6: Commit**

```bash
git add sand/src/physics/gpu.rs sand/src/bin/profile.rs sand/src/physics/tests.rs
git commit -m "feat(sand): per-pass GPU timestamps and a profile binary"
```

---

## Task 3: Indirect dispatch of awake chunks

**Files:**
- Modify: `sand/src/physics/wgsl/wake.wgsl`, `sand/src/physics/wgsl/step.wgsl`, `sand/src/physics/gpu.rs`, `sand/src/physics/cpu.rs` (only if the CPU needs the same list for parity tests; otherwise leave CPU unchanged)
- Create: `sand/src/physics/schedule.rs` (the dilation rule as a pure function, unit-tested)
- Test: `sand/src/physics/tests.rs`

**Interfaces:**
- Consumes: `awake: array<u32>` (per chunk, from the wake pass), `chunk_active` stamps, `chunks_x()`/`chunks_y()` helpers in WGSL.
- Produces:
  - WGSL bindings: `dispatch_list: array<u32>` (chunk indices, storage read-write), `dispatch_args: array<u32, 3>` (x, y, z for `dispatch_workgroups_indirect`), both written by `wake`.
  - Rust: `pub fn dilate(awake: &[bool], cw: usize, ch: usize) -> Vec<usize>` in `schedule.rs` returning the chunk indices to dispatch.

**The mapping (decided here, the implementer must not change it):**
At offset `o`, block top-left cells are at `2·tx − o`. A block whose top-left lies in chunk `c` can have cells in `c`, the chunk to its right `c+(1,0)`, below `c+(0,1)`, and diagonal `c+(1,1)`. So chunk `c`'s workgroup must run whenever any of those four chunks is awake. The dispatch set is `D = { c : awake[c] || awake[c+(1,0)] || awake[c+(0,1)] || awake[c+(1,1)] }`. Each dispatched workgroup handles the blocks whose top-left lies in its chunk, using `workgroup_id` → chunk index via `dispatch_list[workgroup_id.x]` and the existing per-thread awake check unchanged. Threads whose cells are all asleep still return early, so `D` being a superset is always safe (waking is safe).

Note: for `o = 1` the first column of blocks starts at cell −1 and belongs to chunk 0 by the "top-left" rule. The existing code handles it by the same per-thread check; keep that.

- [ ] **Step 1: Write the failing unit test for `dilate`**

```rust
#[test]
fn dilate_includes_right_below_and_diagonal_chunks() {
    // 3x3 chunks; only chunk (1,1) awake (index 4).
    let mut awake = vec![false; 9];
    awake[4] = true;
    let d = dilate(&awake, 3, 3);
    // Chunk (1,1) itself, plus the chunks whose block can reach into it:
    // (0,1) via its right neighbour, (1,0) via below, (0,0) via diagonal.
    assert_eq!(d, vec![0, 1, 3, 4]);
}
```

Index check: (0,0)=0, (1,0)=1, (0,1)=3, (1,1)=4 on a 3-wide grid.

- [ ] **Step 2: Run to verify it fails**

Run: `cd sand && cargo test --release dilate_includes`
Expected: FAIL, `dilate` not found.

- [ ] **Step 3: Implement `dilate` in `schedule.rs`.** Add `pub mod schedule; pub use schedule::*;` to `physics/mod.rs`.

- [ ] **Step 4: Run to verify it passes**

Run: `cd sand && cargo test --release dilate_includes`
Expected: PASS.

- [ ] **Step 5: Write the failing equality test for the GPU path at odd frames and straddling blocks**

```rust
#[test]
fn indirect_dispatch_gives_identical_world_to_no_sleeping() {
    // Settling scene, odd and even frames, every chunk border exercised.
    let mut a = settling_scene(200, 150);
    let mut b = a.clone();
    let Some(mut gpu) = gpu_or_skip(&a) else { return };
    let mut cpu = CpuSim::new(&a);
    for _ in 0..600 { cpu.step_no_sleep(&mut a); }
    gpu.step(600);
    gpu.download(&mut b);
    assert_eq!(a.cells, b.cells, "GPU indirect dispatch diverged from CPU");
}
```

Check that `settling_scene` puts a falling grain on a chunk border with the frame parity exercised. If not, add a grain at x = 31 and 32 and run one extra step to shift parity.

- [ ] **Step 6: Run to verify it fails or passes for the right reason**

Run: `cd sand && cargo test --release indirect_dispatch`
Expected: PASS before the change (the non-indirect path is correct); this test guards the change. Record it as the regression guard.

- [ ] **Step 7: Implement the indirect path in `wake.wgsl` and `gpu.rs`.** `wake` writes `dispatch_list` and `dispatch_args` (an atomic counter in `dispatch_args[0]`; `dispatch_args[1] = 1; dispatch_args[2] = 1`). `GpuSim::step` calls `dispatch_workgroups_indirect(&dispatch_args_buffer, 0)` for the step pass. Keep the non-indirect dispatch behind a `const INDIRECT: bool` in `gpu.rs` until Step 8 passes, so the change can be reverted in one line.

Note that `dispatch_workgroups_indirect` needs the args buffer to have `INDIRECT` usage. `dispatch_args` must be written by the GPU before the step pass in the same encoder, which the barrier between passes provides.

- [ ] **Step 8: Run the equality tests**

Run: `cd sand && cargo test --release gpu_gives_identical gpu_matches_cpu sleeping_gives_identical indirect_dispatch`
Expected: PASS.

- [ ] **Step 9: Benchmark.** Run `cargo run --release --bin bench -- 3`. Record busy and settled columns against the Task 0 baseline. Accept only if settled 4K improves and busy 4K does not regress more than 3%. Otherwise set `INDIRECT = false` and record the result in the baseline doc.

- [ ] **Step 10: Commit**

```bash
git add sand/src/physics sand/src/physics/tests.rs
git commit -m "feat(sand): dispatch only the chunks that can run (indirect)"
```

---

## Task 4: Camera (pan and zoom), no physics change

**Files:**
- Create: `sand/src/bin/physics/camera.rs`
- Modify: `sand/src/bin/physics/main.rs` (`world_rect`, paint and probe mapping, input), `sand/src/bin/physics/settings.rs` (zoom limits)
- Test: `sand/src/bin/physics/camera.rs` (unit tests in the same file)

**Interfaces:**
- Consumes: grid size `(usize, usize)`, screen size `(f32, f32)`, `BAR_HEIGHT`.
- Produces:

```rust
pub struct Camera { pub zoom: f32, pub center: Vec2 /* in cells */ }
impl Camera {
    pub fn fit(grid: (usize, usize), screen: (f32, f32)) -> Camera;     // zoom = 1 is today's letterboxed view
    pub fn dest_rect(&self, grid: (usize, usize), screen: (f32, f32)) -> Rect; // where the grid is drawn
    pub fn screen_to_cell(&self, p: Vec2, grid: (usize, usize), screen: (f32, f32)) -> Option<(i32, i32)>;
    pub fn visible_cells(&self, grid: (usize, usize), screen: (f32, f32)) -> (i32, i32, i32, i32); // x0,y0,x1,y1 exclusive
    pub fn clamp(&mut self, grid: (usize, usize), screen: (f32, f32));
    pub fn zoom_at(&mut self, p: Vec2, factor: f32, grid: (usize, usize), screen: (f32, f32));
}
```

`zoom_at` keeps the cell under the pointer fixed. Zoom range: 1.0 (whole grid) to 32.0. `clamp` keeps the visible rectangle inside the grid when zoom > 1 and centres the grid when zoom ≤ 1.

Input decision: the mouse wheel is brush size now (`main.rs`). Zoom uses Ctrl (or Cmd on macOS) plus wheel, or `=`/`-` keys. Pan: middle-button drag, or Space plus left drag when no brush is active. Decide these in this task and record them in the lesson. Do not reuse the wheel without a modifier.

- [ ] **Step 1: Write failing tests in `camera.rs`**

```rust
#[test]
fn zoom_at_keeps_the_cell_under_the_pointer() {
    let grid = (640, 360); let screen = (1280.0, 760.0);
    let mut cam = Camera::fit(grid, screen);
    let p = vec2(400.0, 300.0);
    let before = cam.screen_to_cell(p, grid, screen);
    cam.zoom_at(p, 4.0, grid, screen);
    assert_eq!(cam.screen_to_cell(p, grid, screen), before);
}

#[test]
fn clamp_keeps_view_inside_grid() {
    let grid = (640, 360); let screen = (1280.0, 760.0);
    let mut cam = Camera { zoom: 8.0, center: vec2(0.0, 0.0) };
    cam.clamp(grid, screen);
    let (x0, y0, x1, y1) = cam.visible_cells(grid, screen);
    assert!(x0 >= 0 && y0 >= 0 && x1 <= 640 && y1 <= 360);
}

#[test]
fn fit_matches_the_old_letterboxed_rect() {
    let grid = (640, 360); let screen = (1280.0, 760.0);
    let r = Camera::fit(grid, screen).dest_rect(grid, screen);
    assert_eq!(r.w, 1280.0); assert_eq!(r.h, 720.0);
}

#[test]
fn screen_to_cell_is_none_outside_the_grid() {
    let grid = (640, 360); let screen = (1280.0, 760.0);
    assert_eq!(Camera::fit(grid, screen).screen_to_cell(vec2(5.0, 5.0), grid, screen), None);
}
```

- [ ] **Step 2: Run to verify fail:** `cd sand && cargo test --release --bin physics camera` → FAIL (module missing).

- [ ] **Step 3: Implement `camera.rs`** to satisfy the four tests. `fit` must reproduce `world_rect` exactly so that zoom 1 looks as it does now.

- [ ] **Step 4: Run to verify pass:** same command → PASS.

- [ ] **Step 5: Wire it into `main.rs`.** Replace `world_rect` calls with `camera.dest_rect`, paint and probe with `camera.screen_to_cell`. Add the zoom and pan input from the decision above.

- [ ] **Step 6: Manual check.** Run `cargo run --release --bin physics`, zoom in with the chosen key on a painted pile, confirm the pile under the cursor stays under the cursor and painting lands where the brush shows. Record the result in the commit message.

- [ ] **Step 7: Commit**

```bash
git add sand/src/bin/physics
git commit -m "feat(viewer): pan and zoom camera; paint and probe follow it"
```

---

## Task 5: Paint and probe correctness under zoom

Covers Review Focus 1.

**Files:**
- Modify: `sand/src/bin/physics/main.rs` (paint call site), `sand/src/bin/physics/camera.rs` (tests)
- Test: `sand/src/bin/physics/camera.rs`

**Interfaces:**
- Consumes: `Camera::screen_to_cell`, `paint(world, m, jet_dir, cell, brush_size, edits)` (unchanged signature).
- Produces: nothing new.

- [ ] **Step 1: Failing test:** a round-trip property test: for 200 random camera states (zoom in 1..32, centre in grid), for every cell visible, `screen_to_cell(cell_to_screen(c)) == c`. Add `Camera::cell_to_screen(&self, cell: (i32, i32), grid, screen) -> Vec2` (centre of the cell) first, as part of this step.

- [ ] **Step 2: Run:** `cargo test --release --bin physics camera_round_trip` → FAIL until the function exists.

- [ ] **Step 3: Implement** `cell_to_screen` and make the round-trip pass.

- [ ] **Step 4: Run:** same command → PASS.

- [ ] **Step 5: Commit**

```bash
git add sand/src/bin/physics
git commit -m "fix(viewer): paint and probe use the camera mapping; round-trip test"
```

---

## Task 6: Sub-rectangle readback

**Files:**
- Modify: `sand/src/physics/gpu.rs` (`step_and_readback` takes a rectangle), `sand/src/bin/physics/main.rs`, `sand/src/bin/physics/render.rs` (upload a sub-rectangle into the cell texture)
- Test: `sand/src/physics/tests.rs`

**Interfaces:**
- Consumes: `Camera::visible_cells`.
- Produces: `GpuSim::step_and_readback_rect(&mut self, steps: u32, rect: (u32, u32, u32, u32))` where rect is `(x0, y0, w, h)` in cells. Copies row by row with `copy_buffer_to_buffer` per row (wgpu has no strided buffer copy; if the row count makes this slow, use one compute pass that packs the rectangle into a linear staging buffer). `latest` returns the rectangle with the world; `World` stays full size, only the rectangle is valid, and the frame records which rectangle.

Decision: the packing compute pass is preferred from the start. Per-row copies at 2160 rows are too many submissions.

- [ ] **Step 1: Failing test:** `pipelined_readback_rect_matches_full_readback_inside_rect`: run 100 steps on a 300×200 world, read the full world via `download` and the rectangle `(37, 20, 120, 90)` via `step_and_readback_rect` and `latest`; assert every cell inside the rectangle equals the full download.

- [ ] **Step 2: Run to fail.** `cargo test --release pipelined_readback_rect` → FAIL.

- [ ] **Step 3: Implement the packing pass** `pack_rect` in `wgsl/common.wgsl` (or a new `wgsl/readback.wgsl` joined into `SHADER`): one thread per cell of the rectangle, writes `out[i] = cells[(y0+y)*w + x0+x]`. Add to `SHADER` join in `gpu.rs`.

- [ ] **Step 4: Run to pass.** Same command → PASS. Then `cargo test --release` (all tests) → PASS.

- [ ] **Step 5: Benchmark.** Add a row to bench: 4K readback at full view and at zoom 2× (quarter of the cells). Record in baseline doc. Accept only if zoom 2× readback cost is at least 40% lower.

- [ ] **Step 6: Commit**

```bash
git add sand/src/physics sand/src/bin/physics sand/src/physics/tests.rs
git commit -m "feat(sand): read back only the visible rectangle"
```

---

## Task 7: Visible-chunk mask and frozen off-screen chunks (approximate mode)

Covers Review Focus 2 and 5.

**Files:**
- Modify: `sand/src/physics/schedule.rs` (visible set and freeze rule), `sand/src/physics/gpu.rs` (mask buffer, forced freeze, buffer sync), `sand/src/physics/cpu.rs` (same rule, for the parity test), `sand/src/bin/physics/main.rs` (camera → mask)
- Test: `sand/src/physics/tests.rs`

**Interfaces:**
- Consumes: `dilate`, camera visible rectangle in cells.
- Produces:
  - `pub fn visible_chunks(x0: i32, y0: i32, x1: i32, y1: i32, cw: usize, ch: usize, ring: usize) -> Vec<bool>` (chunks overlapping the rectangle, grown by `ring` chunks).
  - `GpuSim::set_visible(&mut self, mask: &[bool])` and `CpuSim::set_visible(&mut self, mask: &[bool])`. Default mask is all true, which keeps today's exact behaviour.
  - `pub const VIEW_RING: usize = 2;` (chunks). Justified by max speed 1 cell per step: a grain outside the ring needs at least `VIEW_RING * CHUNK = 64` steps (about 53 ms at 1200 steps/s) to reach the view.

**The gotcha, decided here:** a chunk that becomes frozen has its current state in only one of the two ping-pong buffers. The heat pass of a neighbour reads from `cells_in`, which may be the stale one. So on the frame a chunk is frozen, both buffers must be filled with its current state, via the same rewrite used by `wake_all`/`upload`. Implement it as a compute pass `sync_frozen` (copies `cells_in` to `cells_out` for frozen chunks) run before the step that first skips them. On unfreeze, the chunk is marked active in the previous step (as `wake_all` does), so its own neighbours wake for two steps.

**Exactness statement (write this in the lesson, and make the test assert it):** freezing is an approximation only at the edge of the ring. Inside the view, the state matches the exact simulation for at least `VIEW_RING * CHUNK` steps after the mask change, because nothing outside the ring can reach the view in that time.

- [ ] **Step 1: Failing test, the exactness statement:**

```rust
#[test]
fn view_matches_full_simulation_inside_ring_for_the_guaranteed_time() {
    // Fill a 640x360 grid with falling sand far from the view; view = top-left 128x128.
    let mut exact = scene_sand_field(640, 360);
    let mut approx = exact.clone();
    let mask = visible_chunks(0, 0, 128, 128, chunk_dims(640,360).0, chunk_dims(640,360).1, VIEW_RING);
    let mut cpu_exact = CpuSim::new(&exact);
    let mut cpu_approx = CpuSim::new(&approx);
    cpu_approx.set_visible(&mask);
    for _ in 0..(VIEW_RING * CHUNK) {
        cpu_exact.step(&mut exact);
        cpu_approx.step(&mut approx);
    }
    for y in 0..128 { for x in 0..128 {
        let i = y * 640 + x;
        assert_eq!(exact.cells[i], approx.cells[i], "view cell ({x},{y}) diverged");
    }}
}
```

Implement `scene_sand_field` in `tests.rs` (sand sprinkled across the whole grid). Note: this test must also pass at a longer horizon ONLY if the mask rule is right; do not assert longer horizons.

- [ ] **Step 2: Run to fail.** `cargo test --release view_matches_full` → FAIL (`set_visible` missing).

- [ ] **Step 3: Implement** `visible_chunks`, `set_visible` in both backends, the freeze rule (a frozen chunk does not run and is excluded from the wake dilation), and the sync on freeze.

- [ ] **Step 4: Run to pass.** `cargo test --release view_matches_full` → PASS.

- [ ] **Step 5: Failing test for the stale-buffer gotcha (Review Focus 2):**

```rust
#[test]
fn unfrozen_chunk_behaves_like_it_never_stopped() {
    // Sand falling into a chunk that is frozen for 10 steps then unfrozen.
    // The CPU and GPU must agree exactly after unfreezing (both use the same rule).
    let mut w = scene_drop_across_chunk_border(160, 96);
    let mut g = w.clone();
    let mut cpu = CpuSim::new(&w);
    let Some(mut gpu) = gpu_or_skip(&w) else { return };
    let mask_frozen = all_true_except(&w, 1, 1);
    cpu.set_visible(&mask_frozen); gpu.set_visible(&mask_frozen);
    for _ in 0..10 { cpu.step(&mut w); }
    gpu.step(10);
    let mask_all = all_true(&w);
    cpu.set_visible(&mask_all); gpu.set_visible(&mask_all);
    for _ in 0..50 { cpu.step(&mut w); }
    gpu.step(50); gpu.download(&mut g);
    assert_eq!(w.cells, g.cells);
}
```

Helpers `scene_drop_across_chunk_border`, `all_true_except`, `all_true` go in `tests.rs`. (`all_true` and `all_true_except` return `Vec<bool>` of the grid's chunk count.)

- [ ] **Step 6: Run to fail, implement the sync (already in Step 3, verify it is needed by temporarily removing it: test must fail), restore, run to pass.**

- [ ] **Step 7: Failing test for Review Focus 5:** `grid_change_rebuilds_visible_mask`: after `upload` of a different-size world, the mask is reset to all true and chunks newly visible wake within the same frame (assert `awake_chunks()` equals the count of new chunks plus neighbours). Implement in `GpuSim::upload` and `CpuSim::reset`.

- [ ] **Step 8: Run the full suite.** `cd sand && cargo test --release` → all PASS.

- [ ] **Step 9: Commit**

```bash
git add sand/src/physics sand/src/bin/physics sand/src/physics/tests.rs
git commit -m "feat(sand): visible-chunk mask; off-screen chunks freeze with a bounded edge"
```

---

## Task 8: Wire the mask into the viewer and measure zoom gains

**Files:**
- Modify: `sand/src/bin/physics/main.rs` (compute mask from camera each frame; only when the camera or the grid changed), `sand/src/bin/bench.rs` (zoomed-in rows)
- Modify: `sand/src/bin/physics/settings.rs` and `ui.rs` (switch: "Compute only the view" on by default; off = today's exact full compute)

**Interfaces:**
- Consumes: `Camera::visible_cells`, `visible_chunks`, `set_visible`.
- Produces: setting `settings.view_only: bool`.

- [ ] **Step 1: Wire it.** Recompute the mask only when the camera changes (compare the last `(zoom, center, grid, screen)`), and call `set_visible` on both `gpu` and `CpuSim` if one is in use.

- [ ] **Step 2: Bench.** Add to `bench.rs` a zoomed table: 4K busy scene with a mask covering 1/4 and 1/16 of the grid. Run `cargo run --release --bin bench -- 3`. Record.

- [ ] **Step 3: Acceptance.** Mask at 1/4 view must cost at most 40% of full on 4K busy scene. If it does not, the awake count is dominated by the ring (too wide) or by the dilation; record why before merging.

- [ ] **Step 4: Manual check.** Zoom into a pile, toggle the switch, confirm the pile inside view is visually unchanged for the first 64 steps after a camera move, and that sand at the edge stops rather than vanishes.

- [ ] **Step 5: Commit**

```bash
git add sand/src/bin
git commit -m "feat(viewer): compute only the visible region (switchable); zoomed bench rows"
```

---

## Task 9: Heat cadence (only if Task 1 shows a worthwhile ceiling)

**Gate:** proceed only if the Task 1 heat-off spike saves at least 25% at 1080p. Otherwise skip and mark skipped in the baseline doc.

**Files:**
- Modify: `sand/src/physics/wgsl/common.wgsl` (`const HEAT_EVERY: u32`), `sand/src/physics/wgsl/step.wgsl`, `sand/src/physics/cpu.rs`, `sand/src/physics/thermal.rs`
- Test: `sand/src/physics/tests.rs`

**Interfaces:**
- Consumes: `frame`.
- Produces: `pub const HEAT_EVERY: u32 = 2;` in Rust and WGSL (must match; a test reads both and compares).

The rule: heat is applied in steps where `frame % HEAT_EVERY == 0`; motion always runs. This changes the physics, so it is an approximation of the heat rate. Conservation of heat (`conduction_conserves_heat`) must still hold because each heat step is itself conservative.

- [ ] **Step 1: Failing test:** `heat_every_is_the_same_on_cpu_and_gpu` (exact parity with the new rule) and `heat_every_still_conserves_heat` (run `conduction_conserves_heat` scenario with the rule).

- [ ] **Step 2: Run to fail.** `cargo test --release heat_every` → FAIL.

- [ ] **Step 3: Implement** in both backends. The heat branch in `step.wgsl` is guarded by `if (frame % HEAT_EVERY == 0u)`.

- [ ] **Step 4: Run to pass.** `cargo test --release` → PASS.

- [ ] **Step 5: Measure** with the bench at 1080p busy. Accept only if it improves ≥ 15% and the lava/boil/ice tests (`lava_heats_water_and_turns_to_stone`, `phase_transition_latent_heat`, `ice_melts_in_hot_water`) still pass. Hot-phase tests are the reason this is risky; if any fails, do not merge.

- [ ] **Step 6: Commit**

```bash
git add sand/src/physics
git commit -m "perf(sand): apply heat every N steps (measured; heat-phase tests pass)"
```

---

## Task 10: Kernel micro-optimisation

**Gate:** start only after Task 2 shows where the time goes per pass and after Task 1's RNG spike. If the RNG spike shows the hash is under 10%, skip the RNG part.

**Files:**
- Modify: `sand/src/physics/wgsl/common.wgsl`, `sand/src/physics/wgsl/thermal.wgsl`, `sand/src/physics/wgsl/motion.wgsl`, `sand/src/physics/wgsl/step.wgsl`
- Test: existing equality tests (no new tests needed; they are the guard).

**Interfaces:**
- Consumes: packed cell layout (`material` in low bits, `aux`, `temperature`), `RULES` table in `materials.rs` mirrored in WGSL.
- Produces: unchanged functions and signatures.

Candidates, in the order the profile says to take them (not all of them are expected to pay):
- Pack the material property table into a `vec4<u32>` array read once per thread, not per neighbour (lesson 14 already uses `vec4` for tables; check).
- Hoist repeated `material()` and `temperature()` extraction for the 4 neighbours.
- Replace the fast-path "all one value" test with a precomputed per-workgroup flag where the sleeping check already reads the chunk.

- [ ] **Step 1:** For each candidate, one change, one commit, bench at 1080p and 4K busy. Keep only changes that improve ≥ 3%.

- [ ] **Step 2: Run the equality suite after each commit:** `cd sand && cargo test --release` → PASS.

- [ ] **Step 3: Record the final ms/step** in the baseline doc against the 0.69 ms copy floor.

Commit per candidate:

```bash
git commit -am "perf(sand): <candidate> (<ms before> -> <ms after>, 1080p busy)"
```

---

## Task 11: Audio analysis cost (only if measured above 1 ms)

**Gate:** time `Analyzer::analyze` on a 4K world in `profile`. If under 1 ms, skip and record.

**Files:**
- Modify: `sand/src/bin/profile.rs` (add the timing), `sand/src/audio/analysis.rs` (only if the gate is passed)
- Test: `sand/src/audio/analysis.rs` (existing analysis tests)

**Interfaces:**
- Consumes: `Analyzer::analyze(&mut self, world: &World, frame_seconds: f32, clock_speed: f32, volume: f32) -> AudioFrame`.
- Produces: unchanged signature. If moved to GPU: `Analyzer` reads a reduction result from `GpuSim::audio_stats()` instead of the world.

- [ ] **Step 1:** Add timing to `profile`. Run it.
- [ ] **Step 2:** If over 1 ms, first try a cheaper CPU change (skip cells whose material is in a band that has no event kinds). Only then consider a GPU reduction with the same output.
- [ ] **Step 3:** Existing analysis tests must pass unchanged: `cargo test --release audio`.
- [ ] **Step 4: Commit** `perf(audio): <change> (<ms before> -> <ms after>)`.

---

## Task 12: Off-screen time slicing (optional, approximate, separate setting)

Only after Task 8 is accepted. Off-screen chunks in the ring beyond the view run every `K` steps instead of never; this gives sand in the far field a slow but moving history.

**Files:**
- Modify: `sand/src/physics/schedule.rs`, `sand/src/physics/gpu.rs`, `sand/src/physics/cpu.rs`, `sand/src/bin/physics/settings.rs`
- Test: `sand/src/physics/tests.rs`

**Interfaces:**
- Consumes: `visible_chunks`, `set_visible`.
- Produces: `pub fn slice_mask(visible: &[bool], frame: u32, k: u32) -> Vec<bool>`: visible chunks always run; off-screen chunk `c` runs when `(frame + c) % k == 0`.

Approximation statement: the update is no longer conservative across slice boundaries. Mass is conserved (cells move only inside blocks that run), heat is not exactly conserved between a running and a paused neighbour. The conservation test is therefore run only for the exact mode; in this mode the test asserts mass conservation only.

- [ ] **Step 1: Failing test:** `slice_mask_runs_visible_every_step_and_offscreen_every_k`.
- [ ] **Step 2: Failing test:** `sliced_mode_conserves_mass` (count of every non-heat material before and after N steps equal, with a sealed box).
- [ ] **Step 3: Implement** in both backends, behind the setting. Default off.
- [ ] **Step 4: Run the suite; commit** `feat(sand): optional off-screen time slicing (approximate, mass-conserving)`.

---

## Task 13: Lesson and docs

**Files:**
- Create: `docs/lessons/21-indirect-dispatch-and-the-viewport.md`
- Modify: `docs/lessons/00-index.md` (add the lesson, the dependency edges, and the table row), and fix the stale row at line 238 that says "not yet committed" (see the earlier review)
- Modify: `docs/plans/perf-baseline-2026-11-05.md` (final numbers)

Follow the repo's teaching convention (`learn/` skills: `teach`, `visualize`). Keep the lesson's numbers the same as the baseline doc.

- [ ] **Step 1:** Write lesson 21 covering Tasks 3, 6, 7 and 8, with the exactness statement and the measured table.
- [ ] **Step 2:** Update the index.
- [ ] **Step 3: Commit** `docs(lessons): 21 indirect dispatch and the viewport; index update`.

---

## Final verification

- [ ] `cd sand && cargo test --release` → all PASS.
- [ ] `cd sand && cargo run --release --bin bench -- 3` → compare to baseline; record in the lesson.
- [ ] `cd sand && cargo run --release --bin physics` → zoom, pan, paint, probe, audio on, view-only switch on and off, resize window, change scene: no panic, no stuck sand, no wrong cell under the pointer.

## Decisions this plan made (for the reviewer to check)

1. Exact physics stays the default. Viewport freezing and slicing are approximations, each with a stated, tested bound.
2. The dispatch set uses the top-left-owner rule (`dilate`), not the per-thread wake check alone, so straddling blocks at odd frames cannot be dropped.
3. Off-screen chunks freeze with a sync copy into both ping-pong buffers, rather than running a sparse update that breaks the Margolus invariant.
4. Readback of a rectangle uses a packing compute pass rather than many row copies.
5. Heat cadence and kernel changes wait for measurements from Tasks 1 and 2.
6. Plan saved to `docs/plans/` to match the existing plans in the repo, not `docs/superpowers/plans/`. Change the path if you prefer the skill default.
