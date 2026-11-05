//! Sandbox application: opens a macroquad window and runs the main loop.

use macroquad::prelude::*;
use sandbox_core::{rng::Rng, time::FixedStep, world::World};
use sandbox_render::{buffer::FrameBuffer, presenter::{MacroquadPresenter, Presenter}};
use sandbox_systems::{registry, run_tick};

fn window_conf() -> Conf {
    Conf {
        window_title: "Sandbox".to_owned(),
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut world = World::new(160, 120);
    let mut systems = registry();
    let mut rng = Rng::new(0x5eed);
    let mut step = FixedStep::new();
    let mut fb = FrameBuffer::new(160, 120);
    let mut presenter = MacroquadPresenter::new();
    // macroquad exposes no window handle, so the wgpu presenter renders offscreen only;
    // the macroquad presenter stays the one that shows pixels.
    #[cfg(feature = "gpu")]
    let mut gpu = match sandbox_render::gpu::GpuPresenter::new_offscreen(160, 120) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("gpu presenter disabled: {e}");
            None
        }
    };
    loop {
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        for _ in 0..step.push_frame(get_frame_time()) {
            run_tick(&mut world, &mut systems, &mut rng);
            world.events.clear();
        }
        clear_background(DARKBLUE);
        fb.draw_world(&world, 0);
        presenter.present(&fb);
        #[cfg(feature = "gpu")]
        if let Some(g) = gpu.as_mut() {
            g.present(&fb);
        }
        next_frame().await;
    }
}
