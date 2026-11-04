//! The viewer for the physics engine in `src/physics`.
//!
//!     cargo run --release --bin physics
//!
//! - The simulation runs on the GPU (wgpu compute). Each frame queues the
//!   steps the world clock asks for plus a copy of the grid, and shows the
//!   newest copy that has come back, never waiting for the GPU.
//! - Drawing also runs on the GPU (OpenGL shaders, see `render.rs`): the
//!   CPU only hands over the bytes it got back.
//! - Settings (Esc): quality presets, bloom/lighting toggles, window size,
//!   fullscreen, simulation grid size, GPU budget.
//!
//! Developer aid: `SAND_SCREENSHOT=out.png SAND_FRAMES=120 [SAND_SCENE=1]
//! [SAND_MENU=1] [SAND_HEAT=1]` runs that many frames, saves the screen and
//! exits (used to check the looks without a person watching).

mod render;
mod settings;
mod ui;

use macroquad::prelude::*;
use render::Renderer;
use sand::physics::{clock::Clock, gpu::GpuSim, *};
use settings::{Settings, GRIDS, WINDOWS};
use ui::{Action, Menu, BAR_HEIGHT, BRUSHES};

fn window_conf() -> Conf {
    Conf {
        window_title: "sand physics".to_owned(),
        window_width: WINDOWS[0].0 as i32,
        window_height: WINDOWS[0].1 as i32,
        window_resizable: true,
        // One sample, no depth buffer: everything is drawn as flat quads.
        sample_count: 1,
        // VSync: never draw frames faster than the display shows them.
        platform: miniquad::conf::Platform { swap_interval: Some(1), ..Default::default() },
        ..Default::default()
    }
}

/// Everything that depends on the grid size; rebuilt when it changes.
struct Sim {
    world: World, // the CPU copy, refreshed by readbacks; only used to paint and probe
    gpu: GpuSim,
    renderer: Renderer,
}

impl Sim {
    fn new(scene_index: usize, settings: &Settings) -> Sim {
        let (w, h) = GRIDS[settings.grid];
        let world = scene(scene_index, w, h);
        let mut gpu = GpuSim::new(&world).unwrap_or_else(|e| {
            eprintln!("no usable GPU: {e}");
            std::process::exit(1);
        });
        gpu.set_sleeping(settings.sleeping);
        let mut renderer = Renderer::new((w, h), &settings.graphics);
        renderer.upload(&world.cells);
        Sim { world, gpu, renderer }
    }

    fn replace_world(&mut self, world: World) {
        self.gpu.upload(&world); // also drops readbacks of the old world in flight
        self.renderer.upload(&world.cells);
        self.world = world;
    }
}

/// Where the world is drawn: as large as fits under the toolbar, keeping
/// the grid's aspect ratio (letterboxed).
fn world_rect(grid: (usize, usize)) -> Rect {
    let (sw, sh) = (screen_width(), screen_height() - BAR_HEIGHT);
    let scale = (sw / grid.0 as f32).min(sh / grid.1 as f32);
    let (w, h) = (grid.0 as f32 * scale, grid.1 as f32 * scale);
    Rect::new((sw - w) * 0.5, BAR_HEIGHT + (sh - h) * 0.5, w, h)
}

fn env<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(name).ok()?.parse().ok()
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut settings = Settings::default();
    settings.heat_view = env::<u8>("SAND_HEAT") == Some(1);
    let mut scene_index = env("SAND_SCENE").unwrap_or(0usize).min(SCENES.len() - 1);
    let mut sim = Sim::new(scene_index, &settings);
    let mut clock = Clock::default();
    let mut menu = Menu::new();
    let mut menu_open = env::<u8>("SAND_MENU") == Some(1);
    let mut brush = SAND;
    let mut brush_size = 6;
    let mut paused = false;
    // Reused every frame: no allocations in the steady state.
    let mut edits: Vec<(usize, u32)> = Vec::new();
    let mut actions: Vec<Action> = Vec::new();
    let mut cpu_ms = 0.0f64;
    let screenshot: Option<(String, u32)> = std::env::var("SAND_SCREENSHOT").ok().map(|p| (p, env("SAND_FRAMES").unwrap_or(120)));
    let mut frame = 0u32;

    loop {
        let frame_start = std::time::Instant::now();
        frame += 1;
        actions.clear();

        // --- keyboard ---
        if is_key_pressed(KeyCode::Escape) {
            actions.push(Action::ToggleMenu);
        }
        if is_key_pressed(KeyCode::Tab) || is_key_pressed(KeyCode::Right) {
            actions.push(Action::NextScene);
        }
        if is_key_pressed(KeyCode::Left) {
            actions.push(Action::PreviousScene);
        }
        if is_key_pressed(KeyCode::R) {
            actions.push(Action::Reset);
        }
        if is_key_pressed(KeyCode::C) {
            actions.push(Action::Clear);
        }
        if is_key_pressed(KeyCode::Space) {
            actions.push(Action::TogglePause);
        }
        if is_key_pressed(KeyCode::F11) {
            settings.fullscreen = !settings.fullscreen;
            actions.push(Action::SetFullscreen(settings.fullscreen));
        }
        if is_key_pressed(KeyCode::H) {
            settings.heat_view = !settings.heat_view;
        }
        if is_key_pressed(KeyCode::Period) {
            clock.faster();
        }
        if is_key_pressed(KeyCode::Comma) {
            clock.slower();
        }
        if is_key_pressed(KeyCode::Slash) {
            clock.real_time();
        }
        for (m, key, _) in BRUSHES {
            if is_key_pressed(key) {
                brush = m;
            }
        }
        let wheel = mouse_wheel().1;
        if is_key_pressed(KeyCode::RightBracket) || wheel > 0.0 {
            brush_size = (brush_size + 1).min(40);
        }
        if is_key_pressed(KeyCode::LeftBracket) || wheel < 0.0 {
            brush_size = (brush_size - 1).max(1);
        }

        // --- draw the world (the newest readback) ---
        clear_background(Color::from_rgba(6, 7, 10, 255));
        let grid = GRIDS[settings.grid];
        let dest = world_rect(grid);
        sim.renderer.draw(dest, &settings.graphics, settings.heat_view, get_time() as f32);

        // --- UI on top; it also decides whether the mouse may paint ---
        ui::toolbar(&mut brush, &mut clock, paused, &mut actions);
        if menu_open {
            menu.draw(&mut settings, &mut actions);
        }
        let (mx, my) = mouse_position();
        let over_ui = my < BAR_HEIGHT || (menu_open && menu.wants_mouse());

        // --- carry out what was asked ---
        for &a in &actions {
            match a {
                Action::ToggleMenu => menu_open = !menu_open,
                Action::TogglePause => paused = !paused,
                Action::NextScene | Action::PreviousScene | Action::Reset => {
                    let n = SCENES.len();
                    scene_index = match a {
                        Action::NextScene => (scene_index + 1) % n,
                        Action::PreviousScene => (scene_index + n - 1) % n,
                        _ => scene_index,
                    };
                    sim.replace_world(scene(scene_index, grid.0, grid.1));
                    clock.reset();
                }
                Action::Clear => {
                    sim.replace_world(World::new(grid.0, grid.1));
                    clock.reset();
                }
                Action::SetGrid(_) => {
                    // A new grid size needs new GPU buffers and textures:
                    // build them all fresh; the old ones are freed on drop.
                    sim = Sim::new(scene_index, &settings);
                    clock.reset();
                }
                Action::SetWindow(i) => {
                    if !settings.fullscreen {
                        request_new_screen_size(WINDOWS[i].0, WINDOWS[i].1);
                    }
                }
                Action::SetFullscreen(on) => set_fullscreen(on),
                Action::SetSleeping(on) => sim.gpu.set_sleeping(on),
            }
        }

        // --- paint (left) / erase (right), straight into the GPU's grid ---
        let cell = (((mx - dest.x) / dest.w * grid.0 as f32) as i32, ((my - dest.y) / dest.h * grid.1 as f32) as i32);
        edits.clear();
        if !over_ui {
            let m = if is_mouse_button_down(MouseButton::Left) {
                Some(brush)
            } else if is_mouse_button_down(MouseButton::Right) {
                Some(AIR)
            } else {
                None
            };
            if let Some(m) = m {
                paint(&sim.world, m, cell, brush_size, &mut edits);
            }
        }
        if !edits.is_empty() {
            sim.gpu.write_cells(&edits);
        }

        // --- simulate: as many steps as the world clock needs ---
        let budget = match sim.gpu.gpu_time_ms() {
            Some(ms) if ms > 0.0 => (settings.gpu_budget_ms as f64 / ms).clamp(1.0, 20_000.0) as u32,
            _ => 2_000,
        };
        let mut steps = clock.steps_for_frame(get_frame_time() as f64, budget, paused);
        if paused && is_key_pressed(KeyCode::N) {
            steps = STEPS_PER_SECOND / 60;
            clock.add_steps(steps);
        }
        let start = std::time::Instant::now();
        if steps > 0 || !edits.is_empty() {
            sim.gpu.step_and_readback(steps); // returns at once; the GPU works in the background
        }
        if sim.gpu.latest(&mut sim.world) {
            sim.renderer.upload(&sim.world.cells); // drawn next frame
        }
        let now = start.elapsed().as_secs_f64() * 1000.0;
        cpu_ms = if cpu_ms == 0.0 { now } else { cpu_ms * 0.95 + now * 0.05 };

        // --- overlays ---
        if !over_ui && dest.contains(vec2(mx, my)) {
            let r = brush_size as f32 * dest.w / grid.0 as f32;
            draw_circle_lines(mx, my, r, 1.0, Color::from_rgba(255, 255, 255, 90));
        }
        let (name, about) = SCENES[scene_index];
        draw_text(&format!("{name}: {about}"), dest.x + 12.0, dest.y + 24.0, 18.0, Color::from_rgba(220, 225, 235, 200));
        let bar = (0.1 / CELL_METERS) as f32 * dest.w / grid.0 as f32;
        let (bx, by) = (dest.x + dest.w - bar - 16.0, dest.y + dest.h - 16.0);
        draw_line(bx, by, bx + bar, by, 2.0, Color::from_rgba(255, 255, 255, 180));
        draw_text("10 cm", bx, by - 6.0, 16.0, Color::from_rgba(255, 255, 255, 180));
        if settings.show_stats {
            let probe = sim.world.index(cell.0, cell.1).map_or(String::new(), |i| {
                let c = sim.world.cells[i];
                format!("  |  {} {:.0}°C", NAMES[material(c) as usize], sens(c) as f32 / T_SCALE as f32)
            });
            let gpu_ms = sim.gpu.gpu_time_ms().map_or("n/a".into(), |ms| format!("{ms:.3}"));
            let speed = if paused {
                "paused".to_string()
            } else if clock.behind {
                format!("{}x asked, {:.2}x reached (GPU budget)", clock.speed(), clock.achieved)
            } else {
                format!("{}x ({:.2}x reached)", clock.speed(), clock.achieved)
            };
            ui::stats(&[
                format!("{} fps  |  frame {:.1} ms  |  cpu {:.2} ms", get_fps(), get_frame_time() * 1000.0, cpu_ms),
                format!("gpu {gpu_ms} ms/step  |  {steps} steps/frame"),
                format!("world {:.2} s  |  {speed}", clock.world_seconds),
                format!("grid {}x{}  |  {}  |  brush {}{probe}", grid.0, grid.1, settings::QUALITY_NAMES[settings.quality.index()], brush_size),
            ]);
        }

        if let Some((path, frames)) = &screenshot {
            if frame == *frames {
                // Note: macroquad draws its UI windows inside next_frame, so
                // the settings window is not in the picture.
                get_screen_data().export_png(path);
                std::process::exit(0);
            }
        }
        // Optional frame cap on top of vsync: sleep away the rest of the frame
        // (less heat and battery; the world clock still runs at real speed).
        if let Some(fps) = settings::FPS_CAPS[settings.fps_cap] {
            let target = std::time::Duration::from_secs_f64(1.0 / fps as f64);
            if let Some(rest) = target.checked_sub(frame_start.elapsed()) {
                std::thread::sleep(rest);
            }
        }
        next_frame().await;
    }
}

/// Cells of a round brush, as `(cell index, new cell)` edits in index order.
fn paint(world: &World, m: u32, (cx, cy): (i32, i32), size: i32, edits: &mut Vec<(usize, u32)>) {
    let loose = KIND[m as usize] != SOLID;
    let cell = pack(m, default_aux(m), default_temp(m));
    for dy in -size..=size {
        for dx in -size..=size {
            if dx * dx + dy * dy > size * size {
                continue;
            }
            // Sprinkle loose materials instead of painting solid blobs.
            if loose && m != AIR && rand::gen_range(0, 3) != 0 {
                continue;
            }
            if let Some(i) = world.index(cx + dx, cy + dy) {
                edits.push((i, cell));
            }
        }
    }
}
