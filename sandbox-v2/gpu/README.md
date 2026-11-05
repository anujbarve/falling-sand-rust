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
