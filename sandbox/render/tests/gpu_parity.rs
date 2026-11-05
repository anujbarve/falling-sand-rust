//! GPU presenter tests. Compiled only with `--features gpu`; they skip (loudly) when the
//! machine has no GPU adapter, so a headless CI run never fakes a pass.
#![cfg(feature = "gpu")]

use sandbox_render::buffer::FrameBuffer;
use sandbox_render::gpu::{GpuError, GpuPresenter};
use sandbox_render::presenter::Presenter;

fn golden(w: usize, h: usize) -> FrameBuffer {
    let mut fb = FrameBuffer::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let v = (x as u32).wrapping_mul(2654435761) ^ (y as u32).wrapping_mul(40503);
            fb.pixels[i] = (x * 255 / (w - 1)) as u8;
            fb.pixels[i + 1] = (y * 255 / (h - 1)) as u8;
            fb.pixels[i + 2] = (v >> 8) as u8;
            fb.pixels[i + 3] = if (x + y) % 7 == 0 { 0 } else { 255 };
        }
    }
    fb
}

fn presenter(w: u32, h: u32) -> Option<GpuPresenter> {
    match GpuPresenter::new_offscreen(w, h) {
        Ok(p) => Some(p),
        Err(GpuError::NoAdapter) => {
            eprintln!("SKIPPED: no GPU adapter available; GPU parity not verified on this machine");
            None
        }
        Err(e) => panic!("{e}"),
    }
}

#[test]
fn gpu_readback_matches_cpu_buffer() {
    let Some(mut p) = presenter(128, 96) else { return };
    let fb = golden(128, 96);
    p.present(&fb);
    let out = p.read_pixels().expect("offscreen readback");
    assert_eq!(out.len(), fb.pixels.len());
    let total = 128 * 96;
    let ok = (0..total)
        .filter(|&i| (0..4).all(|c| (out[i * 4 + c] as i32 - fb.pixels[i * 4 + c] as i32).abs() <= 1))
        .count();
    assert!(ok as f64 / total as f64 >= 0.999, "only {ok}/{total} pixels within 1");
}

#[test]
fn resize_to_zero_skips_frame_without_panic() {
    let Some(mut p) = presenter(128, 96) else { return };
    let fb = golden(128, 96);
    p.present(&fb);
    p.resize(0, 0);
    p.present(&fb);
    assert!(p.read_pixels().is_none());
    p.resize(64, 48);
    p.present(&fb);
    assert_eq!(p.read_pixels().unwrap().len(), 64 * 48 * 4);
}
