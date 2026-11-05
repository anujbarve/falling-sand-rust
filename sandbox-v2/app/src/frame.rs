//! Fixed-step loop and the GPU simulation driver.
//!
//! [`FixedStep`] turns frame times into ticks of `DT = 1/60 s`, at most
//! [`MAX_TICKS_PER_FRAME`] per frame (backlog beyond that is dropped, so a
//! slow frame never snowballs). [`Sim`] owns the device and every pass and
//! runs one tick as ONE encoder with ONE compute pass and ONE submit, in the
//! order: brush strokes (mouse and vent), granular, liquid, air, thermal,
//! fire, lava, particles. (There is no chemistry pass in v2 yet.)
//!
//! Events: the passes count flow, impact, crackle and boom into the grid's
//! GPU counter buffer. Every [`EVENT_PERIOD_TICKS`] ticks (100 ms of sim
//! time) the tick's encoder also copies the counters (and the tick's GPU
//! timestamps, when supported) into a staging buffer and clears them; the
//! staging buffer is mapped asynchronously and collected by [`Sim::poll`]
//! without blocking. If the previous copy is still in flight the copy is
//! skipped and the counts carry over to the next one.
//!
//! Device loss: a lost-device callback sets a flag; while lost every GPU
//! call is skipped (never panics). [`Sim::recover`] rebuilds everything on a
//! new [`Gpu`] and reloads the last save if one exists, else the scene.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use sandbox_v2_gpu::materials::Material;
use sandbox_v2_gpu::reference::cell_hash;
use sandbox_v2_gpu::units::{ticks, DT};
use sandbox_v2_gpu::{
    AirPass, BrushPass, CellRaw, FirePass, Gpu, GranularPass, GridError, GridState, LavaPass,
    LiquidPass, ParticleRaw, Particles, Stroke, ThermalPass, EVENT_KINDS,
};

use crate::controls::stroke;
use crate::save::{self, Snapshot};
use crate::scenes::{build, Scene, SceneConfig};

/// Most ticks run for one rendered frame.
pub const MAX_TICKS_PER_FRAME: u32 = 4;
/// Ticks between two event readbacks (6 ticks = 100 ms).
pub const EVENT_PERIOD_TICKS: u32 = 6;
/// Embers thrown per vent emission.
const EMBERS_PER_EMIT: u32 = 3;
/// Staging layout: 4 event counters (16 bytes), then 2 timestamps (16 bytes).
const STAGING_BYTES: u64 = 32;

/// Fixed-timestep accumulator.
#[derive(Clone, Copy, Debug, Default)]
pub struct FixedStep {
    acc: f64,
}

impl FixedStep {
    /// An empty accumulator.
    pub fn new() -> FixedStep {
        FixedStep::default()
    }

    /// Adds a frame of `frame_dt` seconds (non-finite or negative counts as
    /// 0) and returns the ticks to run now, at most [`MAX_TICKS_PER_FRAME`].
    /// When capped, the remaining backlog is dropped.
    pub fn push(&mut self, frame_dt: f32) -> u32 {
        let dt = if frame_dt.is_finite() && frame_dt > 0.0 {
            f64::from(frame_dt)
        } else {
            0.0
        };
        let step = f64::from(DT);
        self.acc += dt;
        let n = (self.acc / step)
            .floor()
            .min(f64::from(MAX_TICKS_PER_FRAME)) as u32;
        self.acc -= f64::from(n) * step;
        if n == MAX_TICKS_PER_FRAME && self.acc >= step {
            self.acc = 0.0;
        }
        n
    }
}

/// State of the event staging buffer.
enum MapState {
    Idle,
    Pending,
    Done(bool),
}

/// GPU timestamp queries around the tick's compute pass.
struct Timer {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    period_ns: f32,
}

/// Everything tied to one device.
struct Engine {
    gpu: Gpu,
    granular: GranularPass,
    liquid: LiquidPass,
    air: AirPass,
    thermal: ThermalPass,
    fire: FirePass,
    lava: LavaPass,
    brush: BrushPass,
    grid: GridState,
    parts: Particles,
    staging: wgpu::Buffer,
    map: Arc<Mutex<MapState>>,
    timer: Option<Timer>,
    lost: Arc<AtomicBool>,
    errors: Arc<AtomicU32>,
}

impl Engine {
    fn new(gpu: Gpu, w: u32, h: u32, cap: u32) -> Result<Engine, GridError> {
        let lost = Arc::new(AtomicBool::new(false));
        let errors = Arc::new(AtomicU32::new(0));
        {
            let lost = lost.clone();
            gpu.device.set_device_lost_callback(move |reason, msg| {
                eprintln!("GPU device lost ({reason:?}): {msg}");
                lost.store(true, Ordering::SeqCst);
            });
            let errors = errors.clone();
            gpu.device
                .on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
                    if errors.fetch_add(1, Ordering::Relaxed) < 8 {
                        eprintln!("GPU error: {e}");
                    }
                }));
        }
        let grid = GridState::new(&gpu, w, h)?;
        let timer = gpu
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
            .then(|| Timer {
                set: gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("tick_time"),
                    ty: wgpu::QueryType::Timestamp,
                    count: 2,
                }),
                resolve: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("tick_time_resolve"),
                    size: 16,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                period_ns: gpu.queue.get_timestamp_period(),
            });
        Ok(Engine {
            granular: GranularPass::new(&gpu),
            liquid: LiquidPass::new(&gpu),
            air: AirPass::new(&gpu),
            thermal: ThermalPass::new(&gpu),
            fire: FirePass::new(&gpu),
            lava: LavaPass::new(&gpu),
            brush: BrushPass::new(&gpu),
            parts: Particles::new(&gpu, cap),
            staging: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("event_staging"),
                size: STAGING_BYTES,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            map: Arc::new(Mutex::new(MapState::Idle)),
            grid,
            timer,
            lost,
            errors,
            gpu,
        })
    }

    fn idle(&self) -> bool {
        matches!(*self.map.lock().expect("map state"), MapState::Idle)
    }

    /// Records the copy of counters (and timestamps) into the staging
    /// buffer and the counter clear.
    fn record_readback(&self, enc: &mut wgpu::CommandEncoder) {
        let ev = (EVENT_KINDS * 4) as u64;
        enc.copy_buffer_to_buffer(self.grid.events(), 0, &self.staging, 0, ev);
        enc.clear_buffer(self.grid.events(), 0, None);
        if let Some(t) = &self.timer {
            enc.resolve_query_set(&t.set, 0..2, &t.resolve, 0);
            enc.copy_buffer_to_buffer(&t.resolve, 0, &self.staging, 16, 16);
        }
    }

    /// Starts mapping the staging buffer (after the submit that filled it).
    fn start_map(&self) {
        *self.map.lock().expect("map state") = MapState::Pending;
        let map = self.map.clone();
        self.staging
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                *map.lock().expect("map state") = MapState::Done(r.is_ok());
            });
    }
}

/// The GPU simulation: device, passes, grid, particles, events.
pub struct Sim {
    eng: Engine,
    scene: Scene,
    cfg: SceneConfig,
    vent: Option<(i32, i32)>,
    vent_every: u32,
    tick: u32,
    seed: u32,
    strokes: Vec<Stroke>,
    batches: Vec<[u32; EVENT_KINDS]>,
    totals: [u64; EVENT_KINDS],
    submits: u64,
    gpu_ms: Option<f32>,
    cpu_ms: f32,
    sanitized: u32,
}

impl Sim {
    /// Builds `scene` on a `w` x `h` grid with hash `seed` on `gpu`.
    pub fn new(gpu: Gpu, scene: Scene, w: u32, h: u32, seed: u32) -> Result<Sim, GridError> {
        let cfg = scene.config();
        let eng = Engine::new(gpu, w, h, cfg.particle_cap)?;
        let mut sim = Sim {
            eng,
            scene,
            cfg,
            vent: None,
            vent_every: cfg.vent_period_s.map_or(0, |s| ticks(s).max(1)),
            tick: 0,
            seed,
            strokes: Vec::new(),
            batches: Vec::new(),
            totals: [0; EVENT_KINDS],
            submits: 0,
            gpu_ms: None,
            cpu_ms: 0.0,
            sanitized: 0,
        };
        sim.restart_scene();
        Ok(sim)
    }

    /// Rebuilds the scene's starting state on the current grid size.
    fn restart_scene(&mut self) {
        let (w, h) = (self.eng.grid.width(), self.eng.grid.height());
        let built = build(self.scene, w, h, self.seed);
        self.vent = built.vent;
        self.sanitized += self.eng.grid.upload_sanitized(&self.eng.gpu, &built.cells);
    }

    /// The device in use.
    pub fn gpu(&self) -> &Gpu {
        &self.eng.gpu
    }

    /// The grid (render it with `Renderer::frame`).
    pub fn grid(&self) -> &GridState {
        &self.eng.grid
    }

    /// The particles (render them with `Renderer::frame`).
    pub fn parts(&self) -> &Particles {
        &self.eng.parts
    }

    /// Ticks run so far.
    pub fn tick_count(&self) -> u32 {
        self.tick
    }

    /// Queue submits made by this sim (one per tick, plus blocking
    /// readbacks, loads and paused paints).
    pub fn submits(&self) -> u64 {
        self.submits
    }

    /// Event counts collected so far, by `EventKind`.
    pub fn event_totals(&self) -> [u64; EVENT_KINDS] {
        self.totals
    }

    /// Takes the event batches collected since the last call (one per
    /// readback, i.e. per 100 ms), for audio.
    pub fn take_event_batches(&mut self) -> Vec<[u32; EVENT_KINDS]> {
        std::mem::take(&mut self.batches)
    }

    /// GPU time of the last timed tick in ms, when timestamps are supported.
    pub fn gpu_ms(&self) -> Option<f32> {
        self.gpu_ms
    }

    /// CPU time to record and submit the last tick, ms.
    pub fn cpu_ms(&self) -> f32 {
        self.cpu_ms
    }

    /// Non-finite floats replaced on upload (scene, load, brush) so far.
    pub fn sanitized(&self) -> u32 {
        self.sanitized + self.eng.brush.sanitized()
    }

    /// Uncaptured GPU errors (validation etc.) seen on the current device.
    pub fn gpu_errors(&self) -> u32 {
        self.eng.errors.load(Ordering::Relaxed)
    }

    /// Whether the device has been lost (call [`Sim::recover`]).
    pub fn is_lost(&self) -> bool {
        self.eng.lost.load(Ordering::SeqCst)
    }

    /// Queues a brush stroke for the next tick (or [`Sim::apply_strokes`]).
    pub fn paint(&mut self, s: Stroke) {
        self.strokes.push(s);
    }

    /// Replaces the whole grid with `cells` (sanitising NaN/inf, counted).
    ///
    /// # Panics
    /// If `cells.len()` is not the grid's `w * h`.
    pub fn load_cells(&mut self, cells: &[CellRaw]) {
        if self.is_lost() {
            return;
        }
        self.sanitized += self.eng.grid.upload_sanitized(&self.eng.gpu, cells);
    }

    /// Submits a command buffer to the GPU queue and increments the submit counter.
    /// Every queue submit in `Sim` must route through this helper to guarantee
    /// consistent accounting across ticks, flushes, strokes and readbacks.
    fn submit_encoder(&mut self, enc: wgpu::CommandEncoder) {
        self.eng.gpu.queue.submit(Some(enc.finish()));
        self.submits += 1;
    }

    /// Records an out-of-band GPU submission that bypassed `submit_encoder`
    /// (for example, internal helper submissions during blocking readbacks).
    fn submit_queued(&mut self) {
        self.submits += 1;
    }

    /// Applies queued strokes now without ticking (used while paused).
    pub fn apply_strokes(&mut self) {
        if self.strokes.is_empty() || self.is_lost() {
            return;
        }
        let e = &mut self.eng;
        let mut enc = e.gpu.device.create_command_encoder(&Default::default());
        e.brush.record(
            &e.gpu,
            &mut enc.begin_compute_pass(&Default::default()),
            &e.grid,
            &self.strokes,
        );
        self.submit_encoder(enc);
        self.strokes.clear();
    }

    /// Runs `n` ticks (each one submit), then collects finished readbacks.
    pub fn run(&mut self, n: u32) {
        for _ in 0..n {
            self.tick();
        }
        self.poll();
    }

    /// Adds `frame_dt` to `clock`, runs the ticks it yields (at most
    /// [`MAX_TICKS_PER_FRAME`]) and returns how many ran.
    pub fn advance(&mut self, clock: &mut FixedStep, frame_dt: f32) -> u32 {
        let n = clock.push(frame_dt);
        self.run(n);
        n
    }

    /// Emits the vent's lava and embers when due.
    fn emit(&mut self) {
        let Some((vx, vy)) = self.vent else { return };
        if self.vent_every == 0 || !self.tick.is_multiple_of(self.vent_every) {
            return;
        }
        let r = (self.eng.grid.width() / 128).max(1);
        self.strokes.push(stroke(Material::Lava, vx, vy, r));
        let embers: Vec<ParticleRaw> = (0..EMBERS_PER_EMIT)
            .map(|k| {
                let hsh = cell_hash(self.tick, k, 1, self.seed);
                let sx = (hsh % 400) as f32 - 200.0;
                let sy = -200.0 - ((hsh >> 9) % 200) as f32;
                ParticleRaw::launched(
                    [vx as f32 + 0.5, vy as f32 - 1.5],
                    [sx, sy],
                    Material::Ember as u32,
                    1200.0,
                    self.cfg.ember_life_s,
                )
            })
            .collect();
        self.eng.parts.spawn(&self.eng.gpu, &embers);
    }

    /// Runs one tick: one encoder, one compute pass, one submit.
    pub fn tick(&mut self) {
        if self.is_lost() {
            return;
        }
        let t0 = Instant::now();
        self.emit();
        let boundary = (self.tick + 1).is_multiple_of(EVENT_PERIOD_TICKS) && self.eng.idle();
        let (tick, seed, wind) = (self.tick, self.seed, self.cfg.wind);
        let e = &mut self.eng;
        let mut enc = e.gpu.device.create_command_encoder(&Default::default());
        {
            let tw = e.timer.as_ref().map(|t| wgpu::ComputePassTimestampWrites {
                query_set: &t.set,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            });
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("tick"),
                timestamp_writes: tw,
            });
            let g = &e.gpu;
            e.brush.record(g, &mut pass, &e.grid, &self.strokes);
            e.granular.record(g, &mut pass, &e.grid, tick, seed);
            e.liquid.record(g, &mut pass, &e.grid, tick, seed);
            e.air.record(g, &mut pass, &e.grid, DT, wind);
            e.thermal.record(g, &mut pass, &mut e.grid);
            e.fire.record(g, &mut pass, &e.grid);
            e.lava.record(g, &mut pass, &e.grid, tick, seed);
            e.parts.record_step(g, &mut pass, &e.grid, DT);
        }
        if boundary {
            e.record_readback(&mut enc);
        }
        self.submit_encoder(enc);
        if boundary {
            self.eng.start_map();
        }
        self.strokes.clear();
        self.tick = self.tick.wrapping_add(1);
        self.cpu_ms = t0.elapsed().as_secs_f32() * 1000.0;
    }

    /// Non-blocking: lets wgpu run callbacks and collects a finished event
    /// readback, if any.
    pub fn poll(&mut self) {
        if self.is_lost() {
            return;
        }
        let _ = self.eng.gpu.device.poll(wgpu::PollType::Poll);
        let done = match *self.eng.map.lock().expect("map state") {
            MapState::Done(ok) => Some(ok),
            _ => None,
        };
        let Some(ok) = done else { return };
        if ok {
            let read = self.eng.staging.slice(..).get_mapped_range().ok().map(|v| {
                let words: Vec<u32> = bytemuck::pod_collect_to_vec(&v);
                words
            });
            self.eng.staging.unmap();
            if let Some(w) = read {
                let batch = [w[0], w[1], w[2], w[3]];
                for (t, &c) in self.totals.iter_mut().zip(&batch) {
                    *t += u64::from(c);
                }
                self.batches.push(batch);
                if let Some(t) = &self.eng.timer {
                    let t0 = u64::from(w[4]) | (u64::from(w[5]) << 32);
                    let t1 = u64::from(w[6]) | (u64::from(w[7]) << 32);
                    let ns = t1.saturating_sub(t0) as f64 * f64::from(t.period_ns);
                    self.gpu_ms = Some((ns / 1.0e6) as f32);
                }
            }
        }
        *self.eng.map.lock().expect("map state") = MapState::Idle;
    }

    /// Blocking: waits for any readback in flight, then reads the counters
    /// still on the GPU, so [`Sim::event_totals`] is complete (tests, quit).
    pub fn flush_events(&mut self) {
        if self.is_lost() {
            return;
        }
        if !self.eng.idle() {
            let _ = self
                .eng
                .gpu
                .device
                .poll(wgpu::PollType::wait_indefinitely());
            self.poll();
        }
        let mut enc = self
            .eng
            .gpu
            .device
            .create_command_encoder(&Default::default());
        self.eng.record_readback(&mut enc);
        self.submit_encoder(enc);
        self.eng.start_map();
        let _ = self
            .eng
            .gpu
            .device
            .poll(wgpu::PollType::wait_indefinitely());
        self.poll();
    }

    /// Blocking copy of the cells (`None` while the device is lost).
    pub fn readback(&mut self) -> Option<Vec<CellRaw>> {
        if self.is_lost() {
            return None;
        }
        self.submit_queued();
        self.eng.grid.try_readback(&self.eng.gpu)
    }

    /// Blocking snapshot of the saved state (`None` while lost).
    pub fn snapshot(&mut self) -> Option<Snapshot> {
        let cells = self.readback()?;
        self.submit_queued();
        let air_vel = self
            .eng
            .air
            .try_read_velocity(&self.eng.gpu, &self.eng.grid)?;
        Some(Snapshot {
            w: self.eng.grid.width(),
            h: self.eng.grid.height(),
            cells,
            air_vel,
        })
    }

    /// Saves to `path` in the v1 text format. Returns a message for the HUD.
    pub fn save(&mut self, path: &Path) -> Result<String, String> {
        let snap = self.snapshot().ok_or("save failed: GPU device lost")?;
        save::write(path, &snap).map_err(|e| format!("save failed: {e}"))?;
        Ok(format!("saved {}", path.display()))
    }

    /// Loads `path`. On any error the world is untouched and the message
    /// says why.
    pub fn load(&mut self, path: &Path) -> Result<String, String> {
        if self.is_lost() {
            return Err("load failed: GPU device lost".into());
        }
        let snap = save::read(path).map_err(|e| format!("load failed: {e}"))?;
        self.apply(&snap)?;
        Ok(format!("loaded {}", path.display()))
    }

    /// Replaces the state with `snap`, resizing the grid if needed;
    /// particles are cleared (they are not saved).
    fn apply(&mut self, snap: &Snapshot) -> Result<(), String> {
        let e = &mut self.eng;
        if (snap.w, snap.h) != (e.grid.width(), e.grid.height()) {
            e.grid =
                GridState::new(&e.gpu, snap.w, snap.h).map_err(|x| format!("load failed: {x}"))?;
            // The scene's vent belongs to the scene's grid size.
            self.vent = None;
        }
        e.parts = Particles::new(&e.gpu, self.cfg.particle_cap);
        self.sanitized += e.grid.upload_sanitized(&e.gpu, &snap.cells);
        self.sanitized += e.air.write_velocity(&e.gpu, &e.grid, &snap.air_vel);
        Ok(())
    }

    /// Rebuilds every GPU object on `gpu` after a device loss, then reloads
    /// `save` if it loads, else restarts the scene. Returns the on-screen
    /// message. Never panics.
    pub fn recover(&mut self, gpu: Gpu, save: Option<&Path>) -> String {
        let (w, h) = (self.eng.grid.width(), self.eng.grid.height());
        let eng = match Engine::new(gpu, w, h, self.cfg.particle_cap) {
            Ok(e) => e,
            Err(e) => return format!("GPU device lost; recovery failed: {e}"),
        };
        self.eng = eng;
        self.strokes.clear();
        self.gpu_ms = None;
        let loaded = save.filter(|p| p.exists()).map(|p| {
            save::read(p)
                .map_err(|e| e.to_string())
                .and_then(|s| self.apply(&s))
        });
        match loaded {
            Some(Ok(())) => "GPU device lost; reloaded the last save".into(),
            Some(Err(e)) => {
                self.restart_scene();
                format!("GPU device lost; save unreadable ({e}); scene restarted")
            }
            None => {
                self.restart_scene();
                "GPU device lost; no save, scene restarted".into()
            }
        }
    }
}
