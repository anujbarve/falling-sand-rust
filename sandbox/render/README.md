# sandbox_render

Rendering helpers built on `sandbox_core` data: colours and a CPU pixel buffer.

## Modules

- `palette` - material colours.
  - `base_color(m: Material) -> [u8; 4]` - RGBA from a table indexed by `m as usize`; `Empty` has alpha 0.
  - `cell_color(m, x, y, seed) -> [u8; 4]` - base colour with deterministic noise, at most 15 per channel.
- `buffer` - pixel output.
  - `FrameBuffer { w, h, pixels }` - `new(w, h)` allocates once.
  - `draw_world(&mut self, world: &World, seed: u32)` - writes RGBA for every cell, no per-frame allocation.

- `smooth` - motion smoothing.
  - `lerp(a, b, t) -> f32` - linear blend.
  - `interp_world_alpha(prev, cur, alpha, out)` - per-element blend of two snapshots, `alpha = accumulator / DT`.

- `presenter` - `Presenter` trait (`present(&mut self, &FrameBuffer)`), `MacroquadPresenter` (default, always compiled), `fit_rect` letterbox helper.
- `gpu` (feature `gpu`) - `GpuPresenter`, a wgpu presenter: `new_offscreen(w, h)`, `from_surface(..)`, `resize(w, h)` (0x0 skips frames), `read_pixels()` for offscreen readback. Surface `Lost`/`Outdated` reconfigure and retry once.

## Feature `gpu`

Off by default. Pulls in `wgpu` 30.0.1 and `pollster` 0.4.0 (pinned exactly). Build the app with it via `cargo run -p app --features gpu`.
Macroquad exposes no window handle, so in `app` the wgpu presenter renders offscreen alongside the macroquad presenter.

## Tests

```bash
cargo test -p sandbox_render
```

GPU parity test (needs a GPU adapter; prints `SKIPPED` and passes vacuously without one, check with `-- --nocapture`):

```bash
cargo test -p sandbox_render --features gpu
cargo test -p sandbox_render --features gpu --test gpu_parity -- --nocapture
```
