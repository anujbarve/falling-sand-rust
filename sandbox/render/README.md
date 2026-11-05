# sandbox_render

Rendering helpers built on `sandbox_core` data: colours and a CPU pixel buffer.

## Modules

- `palette` - material colours.
  - `base_color(m: Material) -> [u8; 4]` - RGBA from a table indexed by `m as usize`; `Empty` has alpha 0.
  - `cell_color(m, x, y, seed) -> [u8; 4]` - base colour with deterministic noise, at most 15 per channel.
- `buffer` - pixel output.
  - `FrameBuffer { w, h, pixels }` - `new(w, h)` allocates once.
  - `draw_world(&mut self, world: &World, seed: u32)` - writes RGBA for every cell, no per-frame allocation.

## Tests

```bash
cargo test -p sandbox_render
```
