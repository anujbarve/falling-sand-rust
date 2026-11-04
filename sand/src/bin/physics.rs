//! Window for the physics engine in `src/physics`. Switch between the CPU
//! and GPU backends with G while it runs and watch the timing change.
//!
//!     cargo run --release --bin physics

use macroquad::prelude::*;
use sand::physics::{cpu::CpuSim, gpu::GpuSim, *};

const W: usize = 640;
const H: usize = 360;
const SCALE: f32 = 2.0;

#[derive(Clone, Copy, PartialEq)]
enum Backend {
    CpuOneThread,
    CpuAllCores,
    Gpu,
}

impl Backend {
    fn name(self) -> &'static str {
        match self {
            Backend::CpuOneThread => "cpu, 1 thread",
            Backend::CpuAllCores => "cpu, all cores (rayon)",
            Backend::Gpu => "gpu (wgpu compute)",
        }
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "sand physics".to_owned(),
        window_width: (W as f32 * SCALE) as i32,
        window_height: (H as f32 * SCALE) as i32,
        window_resizable: false,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut scene_index = 0;
    let mut world = scene(scene_index, W, H);
    let mut cpu = CpuSim::new(&world);
    // No GPU? The viewer still runs, with the GPU backend missing.
    let mut gpu = match GpuSim::new(&world) {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("GPU backend unavailable: {e}");
            None
        }
    };
    let mut backend = Backend::CpuAllCores;
    let mut gpu_stale = true; // the world changed on the CPU side: upload before stepping
    let mut brush = SAND;
    let mut brush_size = 6;
    let mut steps_per_frame = 4u32;
    let mut paused = false;
    let mut heat_view = false;
    let mut ms_per_step = 0.0f64; // smoothed

    let mut image = Image::gen_image_color(W as u16, H as u16, BLACK);
    let texture = Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Nearest);

    loop {
        // --- controls ---
        let mut reload = is_key_pressed(KeyCode::R);
        if is_key_pressed(KeyCode::Tab) || is_key_pressed(KeyCode::Right) {
            scene_index = (scene_index + 1) % SCENES.len();
            reload = true;
        }
        if is_key_pressed(KeyCode::Left) {
            scene_index = (scene_index + SCENES.len() - 1) % SCENES.len();
            reload = true;
        }
        if reload {
            world = scene(scene_index, W, H);
            gpu_stale = true;
        }
        if is_key_pressed(KeyCode::C) {
            world = World::new(W, H);
            gpu_stale = true;
        }
        if is_key_pressed(KeyCode::G) {
            backend = match backend {
                Backend::CpuOneThread => Backend::CpuAllCores,
                Backend::CpuAllCores if gpu.is_some() => Backend::Gpu,
                _ => Backend::CpuOneThread,
            };
            gpu_stale = true;
            ms_per_step = 0.0;
        }
        if is_key_pressed(KeyCode::H) {
            heat_view = !heat_view;
        }
        if is_key_pressed(KeyCode::Space) {
            paused = !paused;
        }
        if is_key_pressed(KeyCode::Equal) {
            steps_per_frame = (steps_per_frame * 2).min(64);
        }
        if is_key_pressed(KeyCode::Minus) {
            steps_per_frame = (steps_per_frame / 2).max(1);
        }
        pick_brush(&mut brush);
        let wheel = mouse_wheel().1;
        if is_key_pressed(KeyCode::RightBracket) || wheel > 0.0 {
            brush_size = (brush_size + 1).min(40);
        }
        if is_key_pressed(KeyCode::LeftBracket) || wheel < 0.0 {
            brush_size = (brush_size - 1).max(1);
        }
        gpu_stale |= paint(&mut world, brush, brush_size);

        // --- simulate ---
        let steps = if !paused {
            steps_per_frame
        } else if is_key_pressed(KeyCode::N) {
            1
        } else {
            0
        };
        if steps > 0 {
            let start = std::time::Instant::now();
            match (backend, gpu.as_mut()) {
                (Backend::Gpu, Some(g)) => {
                    if gpu_stale {
                        g.upload(&world);
                        gpu_stale = false;
                    }
                    g.step(steps);
                    g.download(&mut world); // waits for the GPU, then copies the grid back to draw it
                }
                (Backend::CpuOneThread, _) => (0..steps).for_each(|_| cpu.step(&mut world)),
                _ => (0..steps).for_each(|_| cpu.step_parallel(&mut world)),
            }
            let now = start.elapsed().as_secs_f64() * 1000.0 / steps as f64;
            ms_per_step = if ms_per_step == 0.0 { now } else { ms_per_step * 0.95 + now * 0.05 };
        }

        // --- draw ---
        for i in 0..W * H {
            let cell = world.cells[i];
            let rgb = if heat_view { heat_color(temp(cell)) } else { cell_color(cell, temp(cell), i) };
            image.bytes[i * 4..i * 4 + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
        texture.update(&image);
        draw_texture_ex(
            &texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(W as f32 * SCALE, H as f32 * SCALE)),
                ..Default::default()
            },
        );

        let (mx, my) = mouse_position();
        let under_mouse = world.index((mx / SCALE) as i32, (my / SCALE) as i32);
        let probe = under_mouse.map_or(String::new(), |i| {
            format!(
                "under mouse: {} {:.0}°C",
                NAMES[material(world.cells[i]) as usize],
                temp(world.cells[i]) as f32 / T_SCALE as f32
            )
        });
        let (name, about) = SCENES[scene_index];
        let lines = [
            format!("scene {}/{}: {name} - {about}", scene_index + 1, SCENES.len()),
            format!(
                "{} | {} steps/frame | {:.3} ms/step | {} fps | {}{}",
                backend.name(),
                steps_per_frame,
                ms_per_step,
                get_fps(),
                if paused { "PAUSED (N = step) | " } else { "" },
                probe
            ),
            format!(
                "brush: {} size {} | 1 sand 2 water 3 stone 4 erase 5 wood 6 fire 7 oil 8 lava 9 ice 0 steam",
                NAMES[brush as usize], brush_size
            ),
            "G backend | H heat view | - = steps/frame | [ ] size | Tab scene | R reset | C clear | Space pause".to_string(),
        ];
        draw_rectangle(0.0, 0.0, W as f32 * SCALE, 102.0, Color::new(0.0, 0.0, 0.0, 0.6));
        for (i, text) in lines.iter().enumerate() {
            draw_text(text, 10.0, 22.0 + i as f32 * 24.0, 22.0, WHITE);
        }

        next_frame().await;
    }
}

fn pick_brush(brush: &mut u32) {
    let keys = [
        (KeyCode::Key1, SAND),
        (KeyCode::Key2, WATER),
        (KeyCode::Key3, STONE),
        (KeyCode::Key4, AIR),
        (KeyCode::Key5, WOOD),
        (KeyCode::Key6, FIRE),
        (KeyCode::Key7, OIL),
        (KeyCode::Key8, LAVA),
        (KeyCode::Key9, ICE),
        (KeyCode::Key0, STEAM),
    ];
    for (key, m) in keys {
        if is_key_pressed(key) {
            *brush = m;
        }
    }
}

/// Left mouse paints, right mouse erases. Returns true if anything changed.
fn paint(world: &mut World, brush: u32, size: i32) -> bool {
    let m = if is_mouse_button_down(MouseButton::Left) {
        brush
    } else if is_mouse_button_down(MouseButton::Right) {
        AIR
    } else {
        return false;
    };
    let (mx, my) = mouse_position();
    let (cx, cy) = ((mx / SCALE) as i32, (my / SCALE) as i32);
    let loose = KIND[m as usize] != SOLID;
    for dy in -size..=size {
        for dx in -size..=size {
            if dx * dx + dy * dy > size * size {
                continue;
            }
            // Sprinkle loose materials instead of painting solid blobs.
            if loose && m != AIR && rand::gen_range(0, 3) != 0 {
                continue;
            }
            world.set(cx + dx, cy + dy, m);
        }
    }
    true
}

fn lerp(a: [u8; 3], b: [u8; 3], f: f32) -> [u8; 3] {
    let f = f.clamp(0.0, 1.0);
    [0, 1, 2].map(|k| (a[k] as f32 + (b[k] as f32 - a[k] as f32) * f) as u8)
}

fn cell_color(cell: u32, temp: i32, i: usize) -> [u8; 3] {
    let m = material(cell);
    let c = temp as f32 / T_SCALE as f32; // degrees Celsius
    let base = match m {
        AIR => [20, 20, 28],
        STONE => [110, 110, 115],
        SAND => [220, 190, 110],
        WATER => [50, 110, 220],
        OIL => [120, 85, 35],
        LAVA => lerp([150, 25, 10], [255, 210, 70], (c - 800.0) / 800.0),
        STEAM => [170, 180, 195],
        WOOD => [110, 70, 35],
        FIRE => lerp([200, 30, 10], [255, 220, 60], aux(cell) as f32 / 80.0),
        ICE => [185, 225, 255],
        SMOKE => [55, 55, 60],
        _ => [255, 0, 255],
    };
    let rgb = match KIND[m as usize] {
        // Hot air glows faintly, so you can see convection plumes rise.
        GAS if m == AIR => lerp(base, [120, 40, 20], (c - 40.0) / 600.0),
        GAS => base,
        // Solids, powders and liquids glow red, then yellow, when very hot.
        _ if m != LAVA && c > 450.0 => lerp(base, [255, 140, 50], (c - 450.0) / 600.0),
        _ => base,
    };
    // Per-cell brightness noise so materials look grainy.
    let shade = if m == AIR { 0 } else { ((i.wrapping_mul(2654435761)) >> 28) as u8 % 24 };
    rgb.map(|v| v.saturating_sub(shade))
}

/// Thermal camera: blue cold, black room temperature, red, yellow, white hot.
fn heat_color(temp: i32) -> [u8; 3] {
    let c = temp as f32 / T_SCALE as f32;
    if c < 20.0 {
        lerp([10, 10, 20], [60, 140, 255], (20.0 - c) / 60.0)
    } else if c < 200.0 {
        lerp([10, 10, 20], [200, 30, 20], (c - 20.0) / 180.0)
    } else if c < 900.0 {
        lerp([200, 30, 20], [255, 220, 40], (c - 200.0) / 700.0)
    } else {
        lerp([255, 220, 40], [255, 255, 255], (c - 900.0) / 700.0)
    }
}
