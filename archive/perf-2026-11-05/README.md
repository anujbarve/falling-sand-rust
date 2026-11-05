# Performance and viewport work, 2026-11-05 (frozen)

Status: frozen. Nothing here is merged to `main`.

## Where the code is

| branch | head | contents |
|---|---|---|
| `perf-integration` | 7ddd0b5 | kept work: visible mask, Task 10 kernel changes, Task 12 time slicing, viewer V-toggle, lesson 21, bench table (5e23590) |
| `perf-slice` | 5396b31 | Task 12 time slicing (merged into perf-integration) |
| `main` | 891b8d2 | readback rect, audio skip, profile, camera, baseline (Tasks 0, 1, 2, 4-6, 11) |
| `worktree-agent-*` | various | spike and rejected branches: indirect dispatch (c31b64d), heat cadence (40c938a, dfbf60b), others |

Worktrees under `.claude/worktrees/` still exist. Not removed: removing them needs confirmation.

## Files in this folder

- `2026-11-05-performance-and-viewport.md`: the plan (was untracked).
- `perf-baseline-2026-11-05.md`: baseline and all measured tables (was modified, uncommitted).
- `lesson-21-draft.md`: lesson 21 draft; committed version is on `perf-integration`.
- `*.log`, `*.txt`: bench, profile, test, A/B outputs. `ab-*.log` is the Task 10 A/B (main vs perf-integration).
- `ab.sh`: the A/B script.
- `gpu.rs.good`: snapshot of gpu.rs from the slicing work.

## Open items at freeze

- Task 8 1/4-view gate failed (63% of full, plan asks 40%). Not resolved.
- Viewer mask default and the 32 vs 64 step bound need a decision.
- Final physics run, manual check: not done.
- Task 9 heat cadence: not merged (gate not met).
