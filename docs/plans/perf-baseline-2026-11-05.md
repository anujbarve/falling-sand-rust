# Performance baseline, 2026-11-05

**Machine:** Apple M5 (Metal), 10 CPU threads, timestamp queries supported.
**Command:** `cd sand && cargo run --release --bin bench -- 3` (3 s per measurement), run three times on the same machine. Each table below is the **median of the three runs** per column.
**Code:** `gpu-perf` tip `1d133f8` (fast-forwarded onto this worktree branch). The `profile` bin does not exist yet, so the bench was used.

ms = milliseconds per simulation step (lower is better). 60 fps leaves 16.7 ms per frame. 1 step = 1/1200 s of world time.

## Table 1: step cost by grid (median of 3)

| grid | cells | cpu 1 thr (ms) | cpu all (ms) | gpu (ms) | gpu, GPU timestamps (ms) | gpu + readback (ms) | gpu vs 1 thr | gpu vs all | gpu max speed |
|---|---|---|---|---|---|---|---|---|---|
| 640x360 | 0.23M | 14.241 | 4.109 | 0.346 | 0.295 | 0.990 | 41.2x | 11.5x | 2.41x |
| 1280x720 | 0.92M | 43.952 | 13.940 | 1.576 | 1.499 | 2.351 | 27.9x | 8.9x | 0.53x |
| 1920x1080 | 2.07M | 73.268 | 19.050 | 2.977 | 2.944 | 4.236 | 26.3x | 6.4x | 0.28x |
| 2560x1440 | 3.69M | 113.878 | 36.675 | 4.898 | 4.799 | 9.244 | 23.7x | 7.5x | 0.17x |
| 3840x2160 | 8.29M | 438.399 | 78.465 | 9.444 | 9.251 | 13.381 | 46.4x | 8.3x | 0.09x |

## Table 2: chunk sleeping on the GPU (median of 3)

Busy = bench scene 4. Settled = scene 0 after 6000 steps. Awake = awake chunks / total chunks (32x32).

| grid | busy sleep (ms) | busy no-slp (ms) | settled sleep (ms) | settled no-slp (ms) | settled awake |
|---|---|---|---|---|---|
| 640x360 | 0.349 | 0.336 | 0.070 | 0.121 | 98/240 |
| 1280x720 | 1.561 | 1.497 | 0.516 | 0.690 | 357/920 |
| 1920x1080 | 3.242 | 3.180 | 1.592 | 1.804 | 798/2040 |
| 2560x1440 | 4.984 | 5.069 | 2.720 | 3.166 | 1400/3600 |
| 3840x2160 | 9.474 | 10.533 | 6.601 | 7.435 | 3182/8160 |

### Headline rows (1080p and 4K)

| case | 1080p | 4K |
|---|---|---|
| busy, GPU timestamps | 2.944 ms | 9.251 ms |
| busy, sleep on | 3.242 ms | 9.474 ms |
| busy, sleep off | 3.180 ms | 10.533 ms |
| settled, sleep on | 1.592 ms | 6.601 ms |
| settled, sleep off | 1.804 ms | 7.435 ms |

Note: these are about 1.3 to 1.5 times the 4K and 1080p figures quoted from lesson 14 and the plan (6.28 ms and 2.25 ms busy). The code and machine state differ from that measurement, so treat the plan's numbers as older.

## Noise

- Across the three runs, the GPU columns agree to about 5 to 10 percent.
- The CPU columns and the 4K readback column move more: 4K "gpu + readback" ranged from 12.8 to 22.0 ms, and 4K "cpu 1 thr" from 236 to 851 ms.
- No other cargo process was running when the first run started (checked with `ps`). I did not re-check during the later runs, so other load could have added noise.
- The settled sleep-on vs sleep-off gap at 1080p is about 0.2 ms, which is close to the run-to-run spread.

## Spikes

Task 1 (throwaway, not committed). Each spike was one bench run (3 s), compared against the median baseline above. The shader files were reverted with `git checkout -- sand/src/physics/wgsl/` after each spike.

| spike | 1080p busy, GPU timestamps | 4K busy, GPU timestamps | 1080p busy sleep | 4K busy sleep | check |
|---|---|---|---|---|---|
| baseline (median of 3) | 2.944 ms | 9.251 ms | 3.242 ms | 9.474 ms | identical |
| (a) heat pass off, cell kept unchanged in `step.wgsl` | 0.871 ms | 5.401 ms | 1.860 ms | 4.081 ms | MISMATCH (expected: world changes) |
| (b) `hash3` replaced by a constant in `common.wgsl` | 2.566 ms | 8.429 ms | 2.661 ms | 8.761 ms | MISMATCH (expected: world changes) |
| (c) `GROUP_W = GROUP_H = 8` in `common.wgsl` | 2.692 ms | 9.106 ms | 2.998 ms | 9.359 ms | identical |

Reading the spikes:

- **(a) Heat pass.** About 70 percent of the 1080p step (2.94 to 0.87 ms) and about 42 percent of the 4K step (9.25 to 5.40 ms) is heat. This is the ceiling for Task 9. The world changes when heat is off, so the awake chunk counts are not directly comparable. This is an upper bound, not a prediction.
- **(b) RNG.** Removes about 0.38 ms at 1080p (13 percent) and about 0.82 ms at 4K (9 percent). This is the ceiling for the RNG part of Task 10.
- **(c) Workgroup 8.** About 9 percent faster at 1080p and 2 percent at 4K, both within the run-to-run noise from one run. No clear win, so keep 16, as the plan says. The check still passed.

## Readback rectangle (Task 6)

**Machine and command:** same as above, `cargo run --release --bin bench -- 3`, one run (the brief asked for one run after the tests passed). No other bench or cargo run was active when it started.
**Code:** this branch, after the readback rectangle change (`GpuSim::step_and_readback_rect`, packing pass `pack_rect` in `wgsl/readback.wgsl`).

Readback at 4K, 0 steps, so only the readback is timed. Each readback is: queue the copy (packed first when the rectangle is not the whole grid), wait for the GPU, then `latest` copies the rectangle into the CPU world.

| view | rectangle | readback (ms) |
|---|---|---|
| full view | 3840x2160 | 2.781 |
| zoom 2x | 1920x1080 | 1.327 |

Zoom 2x costs 52 percent less than the full view. The acceptance bar was at least 40 percent, so it passes.

Table 1 and table 2 were re-measured in the same run, and they are not the baseline above. They are shown for reference only. The 4K "gpu + readback" column moved from 13.381 ms (baseline) to 12.452 ms here, and the CPU columns moved more (4K cpu 1 thread 438 ms in the baseline, 239 ms here). Treat those as noise from the machine state, not as a result of this change. The GPU step columns did not change in a way this change could cause (4K gpu 9.444 to 9.578 ms).
