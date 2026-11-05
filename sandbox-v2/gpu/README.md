# sandbox_v2_gpu

Purpose: wgpu device creation and window-surface handling for sandbox v2.

Interfaces:
- `Gpu { instance, adapter, device, queue }`
- `Gpu::new_headless() -> Option<Gpu>` (None when no adapter)
- `Gpu::new_windowed(target, w, h) -> Option<(Gpu, WindowSurface)>`
- `WindowSurface::{resize, size, clear_frame}`; zero-size frames are skipped.

Test: `cargo test -p sandbox_v2_gpu` (prints SKIPPED when no GPU adapter).
