//! The GPU backend: the world lives in GPU buffers and each step is one
//! fused compute dispatch (heat and move together, see sim.wgsl). The CPU
//! only records commands; it never touches the cells until you read back.
//!
//! Chunk sleeping (rule and proof in mod.rs): threads whose cells are all in
//! sleeping 32x32 chunks return at once. A second tiny pass, `wake`, turns
//! the per-chunk activity stamps into next step's awake flags. Results are
//! bit-identical to not sleeping.

use super::{chunk_dims, World};
use std::num::NonZeroU64;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

/// Steps that can be recorded into one submission. Each step needs its own
/// copy of the parameters (its frame number) in the uniform buffer.
const MAX_BATCH: u32 = 256;
/// Uniform buffer offsets must be multiples of this (WebGPU's default limit).
const PARAMS_STRIDE: u64 = 256;
/// The shader is split by system (see physics/wgsl/) and joined here: WGSL
/// allows module-level declarations in any order.
const SHADER: &str = concat!(
    include_str!("wgsl/common.wgsl"),
    include_str!("wgsl/thermal.wgsl"),
    include_str!("wgsl/motion.wgsl"),
    include_str!("wgsl/step.wgsl"),
    include_str!("wgsl/wake.wgsl"),
);
const WAKE_WORKGROUP: u32 = 8; // must match @workgroup_size of `wake`

/// Workgroup shape (threads) declared in sim.wgsl as `const GROUP_W: u32 = ..u;`.
/// Each thread owns a 2x2 block, so a workgroup covers 2*W x 2*H cells.
fn shader_const(name: &str) -> u32 {
    let prefix = format!("const {name}: u32 = ");
    SHADER
        .lines()
        .find_map(|l| l.strip_prefix(&prefix))
        .and_then(|rest| rest.trim_end_matches(';').trim_end_matches('u').parse().ok())
        .unwrap_or_else(|| panic!("sim.wgsl must declare {prefix}<n>u;"))
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    width: u32,
    height: u32,
    frame: u32,
    _pad: u32,
}

pub struct GpuSim {
    device: wgpu::Device,
    queue: wgpu::Queue,
    step: wgpu::ComputePipeline,
    tile: (u32, u32), // cells covered by one workgroup of `step`
    wake: wgpu::ComputePipeline,
    params: wgpu::Buffer,
    // Two copies of the grid. The heat pass reads one and writes the other.
    cells: [wgpu::Buffer; 2],
    // groups[i] reads buffers i and writes buffers 1 - i.
    groups: [wgpu::BindGroup; 2],
    // CPU-readable buffer that results are copied into for `download`.
    staging: wgpu::Buffer,
    // Chunk sleeping, one u32 per chunk: the last frame in which the chunk
    // was active, and whether it runs in the next step. Plus a readback
    // buffer for `awake_chunks`.
    active: wgpu::Buffer,
    awake: wgpu::Buffer,
    awake_staging: wgpu::Buffer,
    chunks: (u32, u32),
    sleeping: bool,
    current: usize, // which of the two copies holds the latest state
    width: u32,
    height: u32,
    frame: u32,
    pub adapter_name: String,
    // --- pipelined readback (see `step_and_readback` / `latest`) ---
    readbacks: Vec<Readback>,
    /// Bumped by `upload`: readbacks from an older epoch are thrown away.
    epoch: u64,
    readback_seq: u64,
    delivered_any: bool,
    // --- GPU timestamp queries (None when the adapter cannot do them) ---
    timer: Option<GpuTimer>,
}

impl GpuSim {
    /// Pick a GPU, compile the shader and upload `world`.
    /// Fails (instead of panicking) when the machine has no usable GPU.
    pub fn new(world: &World) -> Result<Self, String> {
        pollster::block_on(Self::new_async(world))
    }

    async fn new_async(world: &World) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let adapter_name = format!("{} ({:?})", adapter.get_info().name, adapter.get_info().backend);
        // Timestamps at the start/end of a pass, if the adapter has them.
        let timestamps = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        // Ask for the adapter's own limits, so big grids (4K) fit in one buffer.
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("sand"),
                required_limits: adapter.limits(),
                required_features: if timestamps { wgpu::Features::TIMESTAMP_QUERY } else { wgpu::Features::empty() },
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physics.wgsl"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let storage = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sim"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        // The offset is chosen per dispatch: one Params per step.
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(size_of::<Params>() as u64),
                    },
                    count: None,
                },
                storage(1, true),
                storage(2, false),
                storage(3, false),
                storage(4, false),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sim"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let step = pipeline("step");
        let tile = (2 * shader_const("GROUP_W"), 2 * shader_const("GROUP_H"));
        let wake = pipeline("wake");

        let cell_count = (world.width * world.height) as u64;
        let buffer = |label: &str, usage: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: cell_count * 4,
                usage,
                mapped_at_creation: false,
            })
        };
        let grid_usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
        let cells = [buffer("cells0", grid_usage), buffer("cells1", grid_usage)];
        let staging = buffer("staging", wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);
        let (cw, ch) = chunk_dims(world.width, world.height);
        let chunk_buffer = |label: &str, usage: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: (cw * ch * 4) as u64,
                usage,
                mapped_at_creation: false,
            })
        };
        let active = chunk_buffer("active", wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST);
        let awake = chunk_buffer("awake", grid_usage);
        let awake_staging = chunk_buffer("awake staging", wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("params"),
            size: MAX_BATCH as u64 * PARAMS_STRIDE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let group = |src: usize| {
            let dst = 1 - src;
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("sim"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &params,
                            offset: 0,
                            size: NonZeroU64::new(size_of::<Params>() as u64),
                        }),
                    },
                    wgpu::BindGroupEntry { binding: 1, resource: cells[src].as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: cells[dst].as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: active.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 4, resource: awake.as_entire_binding() },
                ],
            })
        };
        let groups = [group(0), group(1)];
        let readbacks = (0..READBACK_SLOTS).map(|_| Readback::new(&device, cell_count * 4)).collect();
        let timer = timestamps.then(|| GpuTimer::new(&device, &queue));

        let mut sim = GpuSim {
            device,
            queue,
            step,
            tile,
            wake,
            params,
            cells,
            groups,
            staging,
            active,
            awake,
            awake_staging,
            chunks: (cw as u32, ch as u32),
            sleeping: true,
            current: 0,
            width: world.width as u32,
            height: world.height as u32,
            frame: world.frame,
            adapter_name,
            readbacks,
            epoch: 0,
            readback_seq: 0,
            delivered_any: false,
            timer,
        };
        sim.upload(world);
        Ok(sim)
    }

    /// Replace the GPU's copy of the world (after painting on the CPU side).
    /// Wakes every chunk: the GPU does not know what changed.
    pub fn upload(&mut self, world: &World) {
        assert_eq!((world.width as u32, world.height as u32), (self.width, self.height));
        self.queue.write_buffer(&self.cells[self.current], 0, bytemuck::cast_slice(&world.cells));
        self.frame = world.frame;
        // Readbacks already in flight hold the old world: never show them.
        self.epoch += 1;
        self.wake_all();
    }

    /// Pretend every chunk was active in the step before the next one, so
    /// all chunks run for the next two steps and both buffers get rewritten.
    fn wake_all(&self) {
        let n = (self.chunks.0 * self.chunks.1) as usize;
        let stamps = vec![self.frame.wrapping_sub(1); n];
        self.queue.write_buffer(&self.active, 0, bytemuck::cast_slice(&stamps));
        self.queue.write_buffer(&self.awake, 0, bytemuck::cast_slice(&vec![1u32; n]));
    }

    /// Turn chunk sleeping on (the default) or off. Off runs every chunk
    /// every step; only for measuring what sleeping saves. Same results.
    pub fn set_sleeping(&mut self, on: bool) {
        self.sleeping = on;
        self.wake_all();
    }

    /// Total number of 32x32 chunks.
    pub fn chunk_count(&self) -> usize {
        (self.chunks.0 * self.chunks.1) as usize
    }

    /// How many chunks will run in the next step. Waits for queued work and
    /// reads the flags back, so it is slow: for benchmarks and tests.
    pub fn awake_chunks(&self) -> usize {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&self.awake, 0, &self.awake_staging, 0, self.awake_staging.size());
        self.queue.submit([encoder.finish()]);
        self.awake_staging.map_async(wgpu::MapMode::Read, .., |r| r.expect("map failed"));
        self.wait();
        let count = {
            let view = self.awake_staging.get_mapped_range(..).expect("mapped");
            bytemuck::cast_slice::<u8, u32>(&view).iter().filter(|&&a| a != 0).count()
        };
        self.awake_staging.unmap();
        count
    }

    /// Queue `steps` simulation steps. Returns straight away: the GPU works
    /// in the background. Call `wait` or `download` to wait for it.
    pub fn step(&mut self, mut steps: u32) {
        let (total, timing) = (steps, self.timer_begin(steps));
        while steps > 0 {
            let batch = steps.min(MAX_BATCH);
            steps -= batch;

            // All the frame numbers for this batch, one per 256-byte slot.
            let mut bytes = vec![0u8; (batch as u64 * PARAMS_STRIDE) as usize];
            for s in 0..batch {
                let p = Params { width: self.width, height: self.height, frame: self.frame.wrapping_add(s), _pad: 0 };
                let at = (s as u64 * PARAMS_STRIDE) as usize;
                bytes[at..at + size_of::<Params>()].copy_from_slice(bytemuck::bytes_of(&p));
            }
            self.queue.write_buffer(&self.params, 0, &bytes);

            let mut encoder = self.device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: None,
                    timestamp_writes: self.timestamp_writes(timing, steps + batch == total, steps == 0),
                });
                for s in 0..batch {
                    let offset = [(s as u64 * PARAMS_STRIDE) as u32];
                    let group = &self.groups[self.current];
                    // wgpu puts a barrier between dispatches that touch the same
                    // buffers, so each dispatch sees the previous one's writes.
                    pass.set_bind_group(0, group, &offset);
                    // When the block offset o is 1, threads start one cell before
                    // the grid (see sim.wgsl), so one more cell must be covered.
                    let o = self.frame.wrapping_add(s) & 1;
                    let groups = ((self.width + o).div_ceil(self.tile.0), (self.height + o).div_ceil(self.tile.1));
                    pass.set_pipeline(&self.step);
                    pass.dispatch_workgroups(groups.0, groups.1, 1);
                    if self.sleeping {
                        pass.set_pipeline(&self.wake);
                        pass.dispatch_workgroups(
                            self.chunks.0.div_ceil(WAKE_WORKGROUP),
                            self.chunks.1.div_ceil(WAKE_WORKGROUP),
                            1,
                        );
                    }
                    self.current = 1 - self.current; // the output is now the input
                }
            }
            self.queue.submit([encoder.finish()]);
            self.frame = self.frame.wrapping_add(batch);
        }
        self.timer_end(timing, total);
    }

    /// Block until the GPU has finished all queued work.
    pub fn wait(&self) {
        self.device.poll(wgpu::PollType::wait_indefinitely()).expect("GPU poll failed");
    }

    /// Copy the GPU's world back into `world` (waits for queued steps first).
    pub fn download(&self, world: &mut World) {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&self.cells[self.current], 0, &self.staging, 0, self.staging.size());
        self.queue.submit([encoder.finish()]);

        // Mapping is asynchronous: ask for it, then poll until it is done.
        self.staging.map_async(wgpu::MapMode::Read, .., |r| r.expect("map failed"));
        self.wait();
        {
            let view = self.staging.get_mapped_range(..).expect("mapped");
            world.cells.copy_from_slice(bytemuck::cast_slice(&view));
        } // the view must be dropped before unmapping
        self.staging.unmap();
        world.frame = self.frame;
    }
}

// ---------------------------------------------------------------------------
// Pipelined readback: draw without waiting for the GPU.
//
// `download` copies the grid into one staging buffer and then blocks until
// the GPU is done. `step_and_readback` instead queues the steps plus a copy
// into the next of a few staging buffers and asks for it to be mapped, then
// returns at once. `latest` polls (without waiting) and hands over the
// newest copy that has arrived. The picture is usually one frame behind.
// ---------------------------------------------------------------------------

/// Staging buffers in rotation. 3 = one being drawn from, one in flight,
/// one spare, so the CPU never has to wait for the GPU in steady state.
const READBACK_SLOTS: usize = 3;

// Life of a staging buffer (readback or timer), kept in an AtomicU8 so the
// map callback (which wgpu calls from inside `poll`) can update it.
const FREE: u8 = 0; // unmapped, not used by any queued command
const PENDING: u8 = 1; // copy queued, map requested, not ready yet
const READY: u8 = 2; // mapped: the CPU may read it

fn map_read(buffer: &wgpu::Buffer, state: &Arc<AtomicU8>) {
    state.store(PENDING, Ordering::Release);
    let state = state.clone();
    buffer.map_async(wgpu::MapMode::Read, .., move |r| {
        state.store(if r.is_ok() { READY } else { FREE }, Ordering::Release);
    });
}

struct Readback {
    buffer: wgpu::Buffer,
    state: Arc<AtomicU8>,
    frame: u32,                                // world.frame of the copied grid
    epoch: u64,                                // GpuSim::epoch when it was queued
    seq: u64,                                  // queue order, to find the newest
    submission: Option<wgpu::SubmissionIndex>, // to wait for it if we must
}

impl Readback {
    fn new(device: &wgpu::Device, size: u64) -> Self {
        Readback {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("readback"),
                size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            state: Arc::new(AtomicU8::new(FREE)),
            frame: 0,
            epoch: 0,
            seq: 0,
            submission: None,
        }
    }

    fn state(&self) -> u8 {
        self.state.load(Ordering::Acquire)
    }

    fn release(&mut self) {
        self.buffer.unmap();
        self.state.store(FREE, Ordering::Release);
    }
}

impl GpuSim {
    /// Queue `steps` steps plus a copy of the result into a staging buffer,
    /// and start mapping it. Does not wait for the GPU (unless it is already
    /// `READBACK_SLOTS` frames behind, to stop work piling up). Pick the
    /// result up later with `latest`. `steps` may be 0 (just show paint).
    pub fn step_and_readback(&mut self, steps: u32) {
        self.step(steps);
        let slot = self.claim_readback_slot();

        let mut encoder = self.device.create_command_encoder(&Default::default());
        let rb = &self.readbacks[slot];
        encoder.copy_buffer_to_buffer(&self.cells[self.current], 0, &rb.buffer, 0, rb.buffer.size());
        let submission = self.queue.submit([encoder.finish()]);

        self.readback_seq += 1;
        let rb = &mut self.readbacks[slot];
        rb.frame = self.frame;
        rb.epoch = self.epoch;
        rb.seq = self.readback_seq;
        rb.submission = Some(submission);
        map_read(&rb.buffer, &rb.state);
    }

    /// A staging buffer we may copy into now.
    fn claim_readback_slot(&mut self) -> usize {
        let _ = self.device.poll(wgpu::PollType::Poll);
        if let Some(i) = self.readbacks.iter().position(|r| r.state() == FREE) {
            return i;
        }
        let oldest = |rs: &[Readback], state: u8| (0..rs.len()).filter(|&i| rs[i].state() == state).min_by_key(|&i| rs[i].seq);
        // A copy that arrived but nobody picked up: the one we are about to queue is newer.
        if let Some(i) = oldest(&self.readbacks, READY) {
            self.readbacks[i].release();
            return i;
        }
        // All in flight: the GPU is far behind. Wait for the oldest copy.
        let i = oldest(&self.readbacks, PENDING).expect("readback slots are free, ready or pending");
        let submission = self.readbacks[i].submission.clone();
        self.device
            .poll(wgpu::PollType::Wait { submission_index: submission, timeout: None })
            .expect("GPU poll failed");
        if self.readbacks[i].state() == READY {
            self.readbacks[i].release();
        }
        i
    }

    /// Copy the newest readback that has arrived into `world` and return
    /// true. Returns false (and leaves `world` alone) if nothing new has
    /// arrived. Never waits, except when no readback has ever arrived yet.
    /// Readbacks queued before the last `upload` are dropped, so they can
    /// never undo what was uploaded.
    pub fn latest(&mut self, world: &mut World) -> bool {
        let _ = self.device.poll(wgpu::PollType::Poll);
        let epoch = self.epoch;
        let usable = |r: &Readback| r.epoch == epoch && r.state() == READY;
        if !self.delivered_any && !self.readbacks.iter().any(usable) {
            let newest = self.readbacks.iter().filter(|r| r.epoch == epoch && r.state() == PENDING).max_by_key(|r| r.seq);
            if let Some(r) = newest {
                let submission = r.submission.clone();
                self.device
                    .poll(wgpu::PollType::Wait { submission_index: submission, timeout: None })
                    .expect("GPU poll failed");
            }
        }

        let best = (0..self.readbacks.len()).filter(|&i| usable(&self.readbacks[i])).max_by_key(|&i| self.readbacks[i].seq);
        if let Some(i) = best {
            let rb = &self.readbacks[i];
            {
                let view = rb.buffer.get_mapped_range(..).expect("mapped");
                world.cells.copy_from_slice(bytemuck::cast_slice(&view));
            } // the view must be dropped before unmapping
            world.frame = rb.frame;
            self.delivered_any = true;
        }
        // Everything that has arrived is now either shown or out of date.
        let best_seq = best.map_or(0, |i| self.readbacks[i].seq);
        for rb in &mut self.readbacks {
            if rb.state() == READY && (rb.epoch != epoch || rb.seq <= best_seq) {
                rb.release();
            }
        }
        best.is_some()
    }

    /// Overwrite single cells on the GPU, e.g. paint: `(index, packed cell)`.
    /// Cheaper than `upload` and, unlike it, keeps readbacks in flight: the
    /// edit simply shows up in the next readback. Edits with consecutive
    /// indices are sent as one write.
    pub fn write_cells(&mut self, edits: &[(usize, u32)]) {
        let cells = &self.cells[self.current];
        let queue = &self.queue;
        let flush = |start: usize, run: &mut Vec<u32>| {
            if !run.is_empty() {
                queue.write_buffer(cells, start as u64 * 4, bytemuck::cast_slice(run));
                run.clear();
            }
        };
        let (mut run, mut run_start) = (Vec::new(), 0usize);
        for &(i, cell) in edits {
            assert!(i < (self.width * self.height) as usize, "cell index out of range");
            if run.is_empty() || i != run_start + run.len() {
                flush(run_start, &mut run);
                run_start = i;
            }
            run.push(cell);
        }
        flush(run_start, &mut run);
        // Edited chunks may be asleep, and only the current buffer got the
        // edit: wake everything so both buffers are rewritten.
        if !edits.is_empty() {
            self.wake_all();
        }
    }
}

// ---------------------------------------------------------------------------
// GPU timestamps: how long the GPU itself spent on the steps.
//
// Wall-clock time around `step` + `wait` also counts the CPU recording and
// submitting commands and the round trip to wake us up. With
// `Features::TIMESTAMP_QUERY` the GPU writes a tick count when the first
// compute pass of a `step` call begins and when the last one ends. We resolve
// the two ticks into a buffer, read it back without waiting (like the
// readback above) and turn ticks into ms with `queue.get_timestamp_period()`.
// (Timestamps between dispatches inside one pass would need
// TIMESTAMP_QUERY_INSIDE_PASSES, which Apple GPUs do not have.)
// ---------------------------------------------------------------------------

const TIMER_SLOTS: usize = 4;

struct TimerSlot {
    buffer: wgpu::Buffer,
    state: Arc<AtomicU8>,
    steps: u32,
    seq: u64,
}

struct GpuTimer {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    slots: Vec<TimerSlot>,
    ns_per_tick: f64,
    seq: u64,
    last_seq: u64,
    last_ms_per_step: Option<f64>,
}

impl GpuTimer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let size = 2 * wgpu::QUERY_SIZE as u64;
        let buffer = |label: &str, usage: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false })
        };
        GpuTimer {
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("step timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }),
            resolve: buffer("timestamps resolve", wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC),
            slots: (0..TIMER_SLOTS)
                .map(|_| TimerSlot {
                    buffer: buffer("timestamps readback", wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST),
                    state: Arc::new(AtomicU8::new(FREE)),
                    steps: 0,
                    seq: 0,
                })
                .collect(),
            ns_per_tick: queue.get_timestamp_period() as f64,
            seq: 0,
            last_seq: 0,
            last_ms_per_step: None,
        }
    }

    /// Read every timestamp pair that has arrived; keep the newest.
    fn harvest(&mut self) {
        for slot in &mut self.slots {
            if slot.state.load(Ordering::Acquire) != READY {
                continue;
            }
            let ticks: [u64; 2] = {
                let view = slot.buffer.get_mapped_range(..).expect("mapped");
                bytemuck::pod_read_unaligned(&view[..16])
            };
            slot.buffer.unmap();
            slot.state.store(FREE, Ordering::Release);
            if slot.seq > self.last_seq && ticks[1] > ticks[0] && slot.steps > 0 {
                let ms = (ticks[1] - ticks[0]) as f64 * self.ns_per_tick / 1e6;
                self.last_ms_per_step = Some(ms / slot.steps as f64);
                self.last_seq = slot.seq;
            }
        }
    }
}

impl GpuSim {
    /// Does this GPU report timestamps? If not, `gpu_time_ms` is always None.
    pub fn has_timestamps(&self) -> bool {
        self.timer.is_some()
    }

    /// GPU time per step, in ms, of the most recent `step` call whose
    /// timestamps have come back. Measured on the GPU's own clock, so it
    /// leaves out CPU-side recording, submitting and waiting. Never blocks.
    pub fn gpu_time_ms(&mut self) -> Option<f64> {
        let _ = self.device.poll(wgpu::PollType::Poll);
        let timer = self.timer.as_mut()?;
        timer.harvest();
        timer.last_ms_per_step
    }

    /// Start of `step`: pick a free timer slot, or None to not time this call
    /// (no timestamp support, nothing to run, or all slots still in flight).
    fn timer_begin(&mut self, steps: u32) -> Option<usize> {
        let timer = self.timer.as_mut()?;
        if steps == 0 {
            return None;
        }
        timer.harvest();
        timer.slots.iter().position(|s| s.state.load(Ordering::Acquire) == FREE)
    }

    /// Timestamp writes for one compute pass of a timed `step` call: the
    /// first pass records the start, the last pass the end.
    fn timestamp_writes(&self, timing: Option<usize>, first: bool, last: bool) -> Option<wgpu::ComputePassTimestampWrites<'_>> {
        timing?;
        let timer = self.timer.as_ref()?;
        (first || last).then(|| wgpu::ComputePassTimestampWrites {
            query_set: &timer.queries,
            beginning_of_pass_write_index: first.then_some(0),
            end_of_pass_write_index: last.then_some(1),
        })
    }

    /// End of `step`: resolve the two timestamps and start reading them back.
    fn timer_end(&mut self, timing: Option<usize>, steps: u32) {
        let (Some(i), Some(timer)) = (timing, self.timer.as_mut()) else { return };
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.resolve_query_set(&timer.queries, 0..2, &timer.resolve, 0);
        encoder.copy_buffer_to_buffer(&timer.resolve, 0, &timer.slots[i].buffer, 0, timer.resolve.size());
        self.queue.submit([encoder.finish()]);
        timer.seq += 1;
        let slot = &mut timer.slots[i];
        slot.steps = steps;
        slot.seq = timer.seq;
        map_read(&slot.buffer, &slot.state);
    }
}
