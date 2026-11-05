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

## Granular and liquid passes (row-parity rule)

`src/reference.rs` is the CPU reference that DEFINES GPU behaviour for sand and water; the WGSL kernels
(`src/shaders/{rules,granular,liquid}.wgsl`) must match it exactly. This rule INTENTIONALLY DIFFERS from the
v1 CPU rules in `sandbox/` (no in-row sequential sweep, no `updated` flags, no Rng stream); v1 is not the oracle.

- Per tick: granular (even rows, then odd rows), then liquid (even rows, then odd rows). Gravity is +y.
- Each phase reads only the state at its start (double buffer). A mover goes into a cell that is empty at
  phase start. Conflicts: mover above, then above-left, above-right, then sideways-left, sideways-right. Losers stay.
- Sand: down, else diagonal (hash bit 0 picks the first side, other side is the fallback).
- Water: down, else diagonal, else ONE sideways step per phase (hash bit 1 picks the first side).
- Hash: `cell_hash(tick, x, y, seed)`, murmur3 fmix32 chain in u32 wrapping math, identical in Rust and WGSL.
- A cell can move once per phase, so up to two cells per tick (even phase then odd phase).

API: `GranularPass::new(gpu).dispatch(gpu, &mut grid, tick, seed)`, same for `LiquidPass`; `dispatch_granular` /
`dispatch_liquid` are one-shot wrappers. `reference::Reference` is the CPU model.

Parity test: `cargo test -p sandbox_v2_gpu --test parity_granular_liquid -- --nocapture`
(prints the mismatch count; skips without a GPU). CPU-only reference tests: `cargo test -p sandbox_v2_gpu --test reference`.
