//! Sandbox application: opens a macroquad window and runs the main loop.
//!
//! Usage: `app [scene]` where `scene` is `volcano_island` (default), `forest`,
//! `ocean_storm` or `explosion_lab`.

use app::controls::{map_key, paint, Action, Controls, POLLED_KEYS};
use app::hud::format_line;
use app::platform::{config_dir, file_io_available};
use app::scenes::{build, systems_with, Scene, HEIGHT, WIDTH};
use macroquad::prelude::*;
use sandbox_audio::AudioHandle;
use sandbox_core::config::{load_config, Config};
use sandbox_core::rng::Rng;
use sandbox_core::save::{load_world, save_world};
use sandbox_core::time::FixedStep;
use sandbox_render::{
    buffer::FrameBuffer,
    presenter::{fit_rect, MacroquadPresenter, Presenter},
};
use sandbox_systems::run_tick;
use std::path::PathBuf;

fn window_conf() -> Conf {
    Conf {
        window_title: "Sandbox".to_owned(),
        ..Default::default()
    }
}

/// Finds the scene's config file: next to the working directory, then in the config dir,
/// then in the source tree. Falls back to the first candidate so `load_config` reports it.
fn scene_config_path(scene: Scene) -> PathBuf {
    let rel = scene.config_path();
    let candidates = [
        rel.clone(),
        config_dir().join(&rel),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(&rel),
    ];
    candidates.iter().find(|p| p.exists()).cloned().unwrap_or_else(|| candidates[0].clone())
}

/// Loads the config once; on targets without file IO the defaults are used silently.
fn load_scene_config(scene: Scene) -> (Config, Option<String>) {
    if file_io_available() {
        load_config(&scene_config_path(scene))
    } else {
        (Config::default(), None)
    }
}

fn save_path() -> PathBuf {
    config_dir().join("save.txt")
}

#[macroquad::main(window_conf)]
async fn main() {
    let arg = std::env::args().nth(1);
    let scene = arg.as_deref().and_then(Scene::from_name).unwrap_or_else(|| {
        if let Some(a) = &arg {
            eprintln!("unknown scene '{a}'; using volcano_island");
        }
        Scene::VolcanoIsland
    });
    let (config, mut notice) = load_scene_config(scene);

    let mut world = build(scene, config.seed);
    let mut systems = systems_with(scene, config.wind, config.sea_level);
    let mut rng = Rng::new(config.seed);
    let mut step = FixedStep::new();
    let mut controls = Controls::new(config.brush);
    let mut fb = FrameBuffer::new(WIDTH, HEIGHT);
    let mut presenter = MacroquadPresenter::new();
    let audio = AudioHandle::start();
    let mut tick_ms = 0.0f32;
    // macroquad exposes no window handle, so the wgpu presenter renders offscreen only;
    // the macroquad presenter stays the one that shows pixels.
    #[cfg(feature = "gpu")]
    let mut gpu = match sandbox_render::gpu::GpuPresenter::new_offscreen(WIDTH as u32, HEIGHT as u32) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("gpu presenter disabled: {e}");
            None
        }
    };
    loop {
        // Events -> actions.
        for key in POLLED_KEYS {
            if !is_key_pressed(key) {
                continue;
            }
            let Some(action) = map_key(key).and_then(|a| controls.apply(a)) else { continue };
            if !file_io_available() {
                notice = Some("save and load are unavailable on this platform".to_owned());
                continue;
            }
            match action {
                Action::Save => {
                    let path = save_path();
                    let result = path
                        .parent()
                        .map_or(Ok(()), std::fs::create_dir_all)
                        .and_then(|_| save_world(&world, &path));
                    notice = Some(match result {
                        Ok(()) => format!("saved {}", path.display()),
                        Err(e) => format!("save failed: {e}"),
                    });
                }
                Action::Load => {
                    notice = Some(match load_world(&save_path()) {
                        Ok(w) => {
                            world = w;
                            "loaded".to_owned()
                        }
                        Err(e) => format!("load failed: {e}"),
                    });
                }
                _ => {}
            }
        }
        if controls.quit {
            break;
        }

        // Brush.
        if is_mouse_button_down(MouseButton::Left) {
            let (mx, my) = mouse_position();
            let (rx, ry, rw, rh) = fit_rect(WIDTH, HEIGHT, screen_width(), screen_height());
            if rw > 0.0 && mx >= rx && my >= ry && mx < rx + rw && my < ry + rh {
                let gx = ((mx - rx) / rw * WIDTH as f32) as i32;
                let gy = ((my - ry) / rh * HEIGHT as f32) as i32;
                paint(&mut world, gx, gy, controls.material, controls.brush);
            }
        }

        // Fixed-step simulation; events go to audio, then are cleared (never before sending).
        let frame_dt = get_frame_time();
        let ticks = if controls.paused { 0 } else { step.push_frame(frame_dt) };
        if ticks > 0 {
            let t0 = get_time();
            for _ in 0..ticks {
                run_tick(&mut world, &mut systems, &mut rng);
                audio.send_events(&world.events);
                world.events.clear();
            }
            tick_ms = ((get_time() - t0) * 1000.0) as f32 / ticks as f32;
        }

        // Draw.
        clear_background(DARKBLUE);
        fb.draw_world(&world, config.seed);
        presenter.present(&fb);
        #[cfg(feature = "gpu")]
        if let Some(g) = gpu.as_mut() {
            g.present(&fb);
        }
        draw_text(format_line(get_fps() as f32, tick_ms), 10.0, 20.0, 22.0, WHITE);
        let status = format!(
            "{}  brush {}  {:?}{}",
            scene.name(),
            controls.brush,
            controls.material,
            if controls.paused { "  PAUSED" } else { "" }
        );
        draw_text(&status, 10.0, 42.0, 22.0, WHITE);
        if let Some(n) = &notice {
            draw_text(n, 10.0, 64.0, 20.0, YELLOW);
        }
        next_frame().await;
    }
}
