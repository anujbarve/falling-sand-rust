# Lesson 21: Indirect dispatch and the viewport

> Sources:
> - `c31b64d` on `perf-integration`: indirect dispatch of awake tiles. Shipped with the flag off.
> - `443fcdb` on `main`: read back only the visible rectangle.
> - `30f428d` on `perf-integration`: visible-chunk mask, with off-screen chunks frozen.
>
> Read any of them with `git show <sha> -- sand/src/physics`. The plan is `docs/plans/2026-11-05-performance-and-viewport.md` (Tasks 3, 6, 7 and 8). The numbers are in `docs/plans/perf-baseline-2026-11-05.md`.

---

## 1. What you will learn

Lesson 13 made a sleeping chunk return early on the GPU. Lesson 14 fused the GPU step into one dispatch. This lesson asks a different question: **can the GPU do less work at all?** There are three answers, and they differ in one important way.

1. **Launch only the workgroups that can do something** (indirect dispatch). The result is exact. It was measured and rejected, so it ships switched off.
2. **Copy back only the rectangle on screen** (sub-rectangle readback). The result is exact. It is accepted: at 2x zoom the readback costs about half of the full-grid readback.
3. **Simulate only the chunks in view, and freeze the rest** (visible-chunk mask). This one is an approximation. It has a stated bound, and it is tested against exact physics.

The exactness statement, once, plainly:

> Exact physics is the default. With the default mask, the GPU world is bit-identical to the CPU reference. The view mask is an approximation: inside the view it matches exact physics for the first 32 steps after the mask changes, and the test checks that bound. Nothing else in this lesson changes the physics.

---

## 2. Concepts

### 2.1 Why a sleeping workgroup still costs something

In lesson 13 a sleeping chunk's workgroup reads the chunk's flag and returns. That saves the arithmetic, but the workgroup was still launched and scheduled. Launch size is a number the CPU hands to `dispatch_workgroups`.

The count of awake chunks is only known on the GPU, because the wake pass computes it. To size a normal dispatch from that count, the CPU would have to read it back and wait. That wait is the stall we want to avoid.

**Indirect dispatch** solves this. The wake pass writes the count and the list of work into a GPU buffer. The step then calls `dispatch_workgroups_indirect`, which reads the three numbers `(x, y, z)` from GPU memory. The CPU never reads them.

Commit `c31b64d` adds this in `sand/src/physics/gpu.rs`: the wake pass writes a `dispatch_list` and a 3-word argument buffer, and `step` dispatches from them when `INDIRECT && self.sleeping`. The constant is `const INDIRECT: bool = false;`.

### 2.2 The tile rule, and why it differs from the plan

The plan (Task 3) wrote a rule. A Margolus block is owned by the chunk that holds its top-left cell. A block whose top-left is in chunk $c$ can reach $c$, the chunk to its right, the chunk below, and the diagonal chunk. So chunk $c$ must run when any of those four is awake. Call that `dilate`.

The rule breaks at odd offsets. At offset $o = 1$, block top-left cells sit at $2t_x - 1$. The first block of chunk 0 starts at cell $-1$, which lies in no chunk. The plan's per-row layout (16 threads per row) cannot cover it.

So the commit uses **tiles**: groups of 32×32 cells. A tile runs when a chunk holding one of its cells is awake. At odd offsets that is the chunk itself, plus the chunk to its left and the chunk above, because the first block of a tile starts one cell earlier.

This dispatches a superset of the plan's set. A superset is safe. Waking a chunk that did not need to run changes nothing, because a thread whose four cells are all asleep returns early. The cost of a bigger set is only extra launches.

### 2.3 What the measurement said

Commit `c31b64d` ran one bench (`bench -- 3`) on the same machine as the baseline:

| case | before | after | change |
|---|---|---|---|
| 4K busy, GPU timestamps | 9.251 ms | 9.721 ms | +5.1% |
| 4K settled, sleep on | 6.601 ms | 6.404 ms | -3% |

The acceptance bar was no more than 3% slower on busy. It failed, so `INDIRECT` is `false`. The code stays in the tree behind the flag.

Two things to notice:

- **A change that removes work can still lose.** The indirect path adds work: the wake pass now writes a list, and the step reads it. In a busy world most tiles run anyway, so the list removes little. The commit does not measure this directly; it is the likely explanation.
- **One run is not a verdict.** The baseline doc says GPU columns agree only to about 5 to 10 percent across runs. A +5.1% result, and a -3% result, both sit inside that band. The rule said reject, and rejecting was right. But the honest summary is "no clear gain", not "a 5% loss". Lesson 12 is the reason to be careful with one number.

### 2.4 Read back only what is on screen

Before `443fcdb`, the viewer copied the whole grid every frame to the CPU. Most of it was off-screen, so most of the copy was wasted.

The new function is `step_and_readback_rect(steps, rect)`. A partial rectangle is first packed on the GPU by one compute pass, `pack_rect` in `wgsl/readback.wgsl`. The packed cells go into one linear buffer, so there is one copy instead of one copy per row. `latest` then writes just that rectangle into the CPU world.

The numbers, at 4K with 0 steps (readback only):

| view | rectangle | cells | readback |
|---|---|---|---|
| full | 3840×2160 | 8.29M | 2.781 ms |
| zoom 2x | 1920×1080 | 2.07M | 1.327 ms |

The zoomed rectangle has exactly one quarter of the cells, since $\frac{1920 \cdot 1080}{3840 \cdot 2160} = \frac{1}{4}$. The time fell by 52%, not by 75%. The gap is a cost that does not scale with the number of cells: waiting for the GPU and mapping the buffer. The commit does not measure that cost separately, so treat it as the likely explanation. The acceptance bar was at least 40% saved, and it passed.

### 2.5 Simulate only what is in view

The mask is a per-chunk boolean. A chunk is **visible** if it overlaps the view, grown by `VIEW_RING = 2` chunks on every side. `visible_chunks` builds it from the camera rectangle.

Chunks outside the mask are **frozen**:

- a frozen chunk does not run;
- a frozen chunk does not wake its neighbours;
- a block moves only if **all four** of its cells are in awake chunks, so a frozen chunk is never written.

The mask is all true by default, which is exact physics. Setting a non-default mask is what makes the simulation approximate.

#### The stale-buffer trap

The simulation ping-pongs between two grids (lesson 8). When a chunk freezes, its current state exists in only one of the two buffers. A neighbour's heat pass reads from whichever buffer is "in", so it can read the stale copy.

The fix is a sync. On the frame a chunk freezes, its state is copied into **both** buffers. On the GPU this is the `sync_frozen` pass. On the CPU it is a plain copy. Without the sync, a test with a chunk that freezes and unfreezes diverges between CPU and GPU.

### 2.6 The exactness bound, and where the plan's number was wrong

How long can a view stay exact after its outside is frozen? The bound comes from how far a change can travel in one step.

After one step, a cell depends on cells up to 2 away. Heat reads one neighbour. A 2×2 block then mixes two heat results, so a moved cell can see a cell at distance 2. The plan assumed 1 cell per step. The commit corrected that to 2.

The first frozen cell sits at least $\text{VIEW\_RING} \cdot \text{CHUNK} + 1$ cells beyond the view. A change from there reaches the view only after $t$ steps where

$$
2t \;\ge\; \text{VIEW\_RING} \cdot \text{CHUNK} + 1
$$

With $\text{VIEW\_RING} = 2$ and $\text{CHUNK} = 32$, the view is untouched while $2t < 65$, so $t \le 32$:

$$
\text{VIEW\_EXACT\_STEPS} = \frac{\text{VIEW\_RING} \cdot \text{CHUNK}}{2} = 32
$$

The plan said 64 steps. That number came from the 1-cell-per-step assumption. The code uses 32. The test must check 32.

---

## 3. Measured results

Baseline numbers come from `docs/plans/perf-baseline-2026-11-05.md`. Each row is one run unless the baseline doc says otherwise. The GPU columns vary by about 5 to 10 percent between runs.

| change | case | before (ms) | after (ms) | change | status |
|---|---|---|---|---|---|
| indirect dispatch (`c31b64d`) | 4K busy, GPU timestamps | 9.251 | 9.721 | +5.1% | rejected, flag off |
| indirect dispatch (`c31b64d`) | 4K settled, sleep on | 6.601 | 6.404 | -3% | within noise |
| sub-rect readback (`443fcdb`) | 4K, full view vs zoom 2x | 2.781 | 1.327 | -52% | accepted (bar: 40%) |
| visible mask (`30f428d`) | 4K busy, mask covers 1/4 of grid | 9.251 | TBD-TASK8-1/4 | TBD | Task 8 |
| visible mask (`30f428d`) | 4K busy, mask covers 1/16 of grid | 9.251 | TBD-TASK8-1/16 | TBD | Task 8 |
| kernel micro-opts (Task 10) | 1080p busy, GPU timestamps | 2.944 | TBD-T10-1080 | TBD | Task 10 |
| kernel micro-opts (Task 10) | 4K busy, GPU timestamps | 9.251 | TBD-T10-4K | TBD | Task 10 |

The "before" column for the Task 8 rows is the baseline full-view value. Task 8 should re-measure the full view in the same run before the zoomed rows are compared with it.

---

## 4. Live examples

- **Read the flag.** `grep -n "INDIRECT" sand/src/physics/gpu.rs` shows `const INDIRECT: bool = false;`. Set it to `true` and rerun the equality tests added in `c31b64d` to see the indirect path run; `git show c31b64d -- sand/src/physics/tests.rs` lists them.
- **Watch the readback.** `cargo run --release --bin bench -- 3` prints the 4K readback rows. Compare the full-view and zoomed-view rows.
- **Run the exact-vs-approximate comparison.** Once Task 8 lands, set the viewer's view-only switch on and off, zoom into a pile, and compare the pile inside the view. The first 32 steps after a camera move should match.

---

## 5. Exercises

1. **Bytes versus time.** A 1920×1080 rectangle is a quarter of the 4K cells. Its readback costs 48% of the full readback, not 25%. Write down two costs that do not shrink with the number of cells. Which one would you measure first?
2. **Re-derive the bound.** Set $\text{VIEW\_RING} = 3$ and keep $\text{CHUNK} = 32$. Use the inequality in section 2.6 to get the new `VIEW_EXACT_STEPS`. Check your answer against the formula, not against the plan.
3. **Why a superset is safe.** Explain why running a chunk that did not need to run cannot change the world. Hint: what does a thread do when all four of its cells are asleep?

---

## 6. Common mistakes

- **Copying the plan's 64 steps.** The plan's number assumed 1 cell per step. The bound is 32 steps. A test that checks 64 steps will fail for the wrong reason.
- **Forgetting the sync on freeze.** Without the copy into both buffers, a neighbour reads a stale state. The bug appears only after a chunk unfreezes, so it is easy to miss.
- **Reading the count back to size a dispatch.** That reintroduces the stall indirect dispatch exists to remove.
- **Treating one bench run as a verdict.** The indirect result sits inside the baseline's own noise band. Repeat the run before you call a result a loss or a gain.

---

## 7. Recap

- Indirect dispatch lets the GPU size its own launch from a buffer the wake pass wrote, so the CPU never waits. Its tile rule is a superset of the plan's rule, because the plan's rule does not cover the first block at odd offsets. It was measured at +5.1% on busy, so it ships off.
- Sub-rectangle readback packs the visible cells into one buffer. At 2x zoom it costs 52% less at 4K.
- The visible mask freezes chunks outside the view. It is the one approximation. Inside the view it is exact for `VIEW_EXACT_STEPS = VIEW_RING * CHUNK / 2 = 32` steps after the mask changes.
- Exact physics is the default. The mask and its bound are tested. Everything else in this lesson is exact.

---

## INDEX LINE

**[[21-indirect-dispatch-and-the-viewport]]**: Launching only the GPU work that can do something: indirect dispatch sized on the GPU (measured, and off by default because it was slower), reading back only the rectangle on screen (52% less at 2x zoom at 4K), and freezing off-screen chunks behind a bounded, tested approximation (32 steps). The exact physics stays the default.
*Concepts:* indirect dispatch, supersets as a safe over-approximation, honest reading of one-run benchmarks against a noise band, sub-rectangle GPU packing, ping-pong buffers and stale state, deriving a bound from a per-step dependency reach.

---

### Notes for the author (delete before publishing)

- **Plan vs code, 64 vs 32.** Plan Task 7 says `VIEW_RING * CHUNK = 64` steps. The code (`30f428d`) uses `VIEW_RING * CHUNK / 2 = 32`. The lesson follows the code. Task 8's manual check still says "first 64 steps"; fix it.
- **Plan Task 8 default.** The plan turns "Compute only the view" on by default in the viewer. That makes the viewer approximate by default, which conflicts with the exactness statement in section 1 if the viewer default changes. The lesson says the *engine* default is exact. Decide which default the viewer ships before merging Task 8, and say so in section 1.
- **Duplicate commit on `perf-integration`.** `54adac6` has the same subject as `30f428d` (`feat(sand): visible-chunk mask; ...`). The lesson cites `30f428d` as instructed. Confirm which one is the final one.
- **Tile rule versus plan.** Section 2.2 is written from the `c31b64d` commit message. I did not re-read the code to confirm the exact tile indexing. Check it against `gpu.rs` on `perf-integration` before publishing.
- **Index placement.** `00-index.md` needs this entry under a Part heading (Part 4 after lesson 20, or a new Part 5), a row in the commit table for `c31b64d`, `443fcdb` and `30f428d`, the dependency edges (lessons 13, 15 and 14 lead here), and the stale row near line 238 ("not yet committed") from the plan's Task 13.
- **Benchmark placeholders.** `TBD-TASK8-1/4`, `TBD-TASK8-1/16`, `TBD-T10-1080`, `TBD-T10-4K` are unmeasured. Do not fill them from earlier runs.
