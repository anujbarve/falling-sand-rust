//! Sandbox v2 application: winit window, GPU sim loop, renderer, audio, HUD.
//!
//! Usage: `sandbox_v2_app [scene]` (`volcano_island`, the default; other
//! scene names load their config with a placeholder layout until Task 10).
//!
//! The sim steps in `about_to_wait` (so it keeps running while the window
//! is minimised); rendering happens in `RedrawRequested` and is skipped for
//! a 0x0 window. See `frame` for the loop and the README for controls.

use std::sync::Arc;
use std::time::{Duration, Instant};

use sandbox_audio::AudioHandle;
use sandbox_core::config::{load_config, Config};
use sandbox_v2_app::controls::{cell_at, map_key, stroke, Action, Controls};
use sandbox_v2_app::frame::{FixedStep, Sim};
use sandbox_v2_app::hud::{hud_line, Hud};
use sandbox_v2_app::platform::{config_path, file_io_available, save_path};
use sandbox_v2_app::scenes::Scene;
use sandbox_v2_app::sounds::sounds;
use sandbox_v2_gpu::units::DT;
use sandbox_v2_gpu::{Gpu, Renderer, WindowSurface};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// How long a notice stays in the HUD.
const NOTICE_TIME: Duration = Duration::from_secs(4);
/// How often the HUD (window title) is refreshed.
const HUD_PERIOD: Duration = Duration::from_millis(250);

/// Window-bound GPU objects (rebuilt after a device loss).
struct Present {
    surface: WindowSurface,
    renderer: Renderer,
}

struct App {
    scene: Scene,
    config: Config,
    window: Option<Arc<Window>>,
    present: Option<Present>,
    sim: Option<Sim>,
    audio: AudioHandle,
    controls: Controls,
    clock: FixedStep,
    last: Instant,
    cursor: Option<(f64, f64)>,
    mouse_down: bool,
    notice: Option<(String, Instant)>,
    frames: u32,
    fps: f32,
    hud_at: Instant,
    /// The last redraw presented nothing (minimised, occluded, no frame).
    idle_frame: bool,
}

impl App {
    fn new(scene: Scene, config: Config, notice: Option<String>) -> App {
        let now = Instant::now();
        App {
            scene,
            controls: Controls::new(config.brush),
            config,
            window: None,
            present: None,
            sim: None,
            audio: AudioHandle::start(),
            clock: FixedStep::new(),
            last: now,
            cursor: None,
            mouse_down: false,
            notice: notice.map(|n| (n, now)),
            frames: 0,
            fps: 0.0,
            hud_at: now,
            idle_frame: false,
        }
    }

    fn say(&mut self, msg: String) {
        eprintln!("{msg}");
        self.notice = Some((msg, Instant::now()));
    }

    /// Creates the surface and renderer for `window` on a new device.
    fn connect(window: &Arc<Window>) -> Option<(Gpu, Present)> {
        let size = window.inner_size();
        let (gpu, surface) = Gpu::new_windowed(window.clone(), size.width, size.height)?;
        let renderer = Renderer::new(&gpu, surface.format());
        Some((gpu, Present { surface, renderer }))
    }

    /// Device lost: drop the old surface, rebuild on a new device, reload
    /// the last save. Retries on the next frame if no device is available.
    fn recover(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        self.present = None;
        let Some((gpu, present)) = App::connect(&window) else {
            self.say("GPU device lost; waiting for a device".into());
            return;
        };
        let save = save_path();
        let save = (file_io_available() && save.exists()).then_some(save);
        let Some(sim) = self.sim.as_mut() else { return };
        let msg = sim.recover(gpu, save.as_deref());
        self.present = Some(present);
        self.say(msg);
    }

    fn handle(&mut self, action: Action, event_loop: &ActiveEventLoop) {
        let Some(action) = self.controls.apply(action) else {
            if self.controls.quit {
                event_loop.exit();
            }
            return;
        };
        if !file_io_available() {
            self.say("save and load are unavailable on this platform".into());
            return;
        }
        let Some(sim) = self.sim.as_mut() else { return };
        let path = save_path();
        let r = match action {
            Action::Save => sim.save(&path),
            _ => sim.load(&path),
        };
        self.say(r.unwrap_or_else(|e| e));
    }

    fn update_hud(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.hud_at);
        if dt < HUD_PERIOD {
            return;
        }
        self.fps = self.frames as f32 / dt.as_secs_f32();
        self.frames = 0;
        self.hud_at = now;
        if self
            .notice
            .as_ref()
            .is_some_and(|(_, t)| now.duration_since(*t) > NOTICE_TIME)
        {
            self.notice = None;
        }
        let (Some(sim), Some(window)) = (self.sim.as_ref(), self.window.as_ref()) else {
            return;
        };
        let line = hud_line(&Hud {
            fps: self.fps,
            sim_ms: sim.gpu_ms().unwrap_or(sim.cpu_ms()),
            sim_gpu: sim.gpu_ms().is_some(),
            particles: sim.parts().count(),
            material: self.controls.material,
            brush: self.controls.brush,
            paused: self.controls.paused,
            notice: self.notice.as_ref().map(|(n, _)| n.as_str()),
        });
        window.set_title(&line);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Sandbox v2")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("cannot create window: {e}");
                event_loop.exit();
                return;
            }
        };
        let Some((gpu, present)) = App::connect(&window) else {
            eprintln!("no GPU adapter available");
            event_loop.exit();
            return;
        };
        let cfg = self.scene.config();
        match Sim::new(gpu, self.scene, cfg.w, cfg.h, self.config.seed) {
            Ok(sim) => self.sim = Some(sim),
            Err(e) => {
                eprintln!("cannot create grid: {e}");
                event_loop.exit();
                return;
            }
        }
        self.present = Some(present);
        self.window = Some(window);
        self.last = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state.is_pressed() && !event.repeat =>
            {
                if let Some(a) = map_key(&event.logical_key) {
                    self.handle(a, event_loop);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = Some((position.x, position.y))
            }
            WindowEvent::CursorLeft { .. } => self.cursor = None,
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => self.mouse_down = state == ElementState::Pressed,
            WindowEvent::Resized(size) => {
                if let (Some(p), Some(sim)) = (self.present.as_mut(), self.sim.as_ref()) {
                    p.surface.resize(sim.gpu(), size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                let (Some(p), Some(sim)) = (self.present.as_mut(), self.sim.as_ref()) else {
                    return;
                };
                if sim.is_lost() {
                    return;
                }
                let renderer = &mut p.renderer;
                let drawn = p.surface.render(sim.gpu(), |view| {
                    renderer.frame(sim.gpu(), sim.grid(), sim.parts(), view);
                });
                self.frames += u32::from(drawn);
                self.idle_frame = !drawn;
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32();
        self.last = now;
        let Some(sim) = self.sim.as_mut() else { return };
        if self.mouse_down {
            let win = self.present.as_ref().map_or((0, 0), |p| p.surface.size());
            let grid = (sim.grid().width(), sim.grid().height());
            if let Some((x, y)) = self.cursor.and_then(|(px, py)| cell_at(px, py, win, grid)) {
                sim.paint(stroke(self.controls.material, x, y, self.controls.brush));
            }
        }
        if self.controls.paused {
            sim.apply_strokes();
            sim.poll();
        } else {
            sim.advance(&mut self.clock, dt);
        }
        // Audio never blocks: send_events drops sounds when its queue is full.
        for batch in sim.take_event_batches() {
            self.audio.send_events(&sounds(batch));
        }
        if sim.is_lost() {
            self.recover();
        }
        self.update_hud();
        let drawable = self
            .present
            .as_ref()
            .is_some_and(|p| p.surface.is_drawable());
        if drawable {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
        if drawable && !self.idle_frame {
            event_loop.set_control_flow(ControlFlow::Poll);
        } else {
            // Minimised or occluded: keep ticking at the sim rate without
            // spinning (a redraw is still requested to notice un-occlusion).
            event_loop.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_secs_f32(DT)));
        }
    }
}

fn main() {
    let arg = std::env::args().nth(1);
    let scene = arg
        .as_deref()
        .and_then(Scene::from_name)
        .unwrap_or_else(|| {
            if let Some(a) = &arg {
                eprintln!("unknown scene '{a}'; using volcano_island");
            }
            Scene::VolcanoIsland
        });
    let (config, notice) = if file_io_available() {
        load_config(&config_path())
    } else {
        (Config::default(), None)
    };
    let event_loop = match EventLoop::new() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot create event loop: {e}");
            return;
        }
    };
    event_loop.set_control_flow(ControlFlow::Poll);
    if let Err(e) = event_loop.run_app(&mut App::new(scene, config, notice)) {
        eprintln!("event loop error: {e}");
    }
}
