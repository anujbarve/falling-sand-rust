//! The GPU backend: the world lives in GPU buffers and each step is two
//! compute-shader dispatches (heat, then move). The CPU only records
//! commands; it never touches the cells until you call `download`.
//!
//! Chunk sleeping (rule and proof in mod.rs): every workgroup of the heat and
//! move passes belongs to one 32x32 chunk and returns at once when that chunk
//! is asleep. A third tiny pass, `wake`, turns the per-chunk activity stamps
//! into next step's awake flags. Results are bit-identical to not sleeping.

use super::{chunk_dims, World};
use std::num::NonZeroU64;

/// Steps that can be recorded into one submission. Each step needs its own
/// copy of the parameters (its frame number) in the uniform buffer.
const MAX_BATCH: u32 = 256;
/// Uniform buffer offsets must be multiples of this (WebGPU's default limit).
const PARAMS_STRIDE: u64 = 256;
const WORKGROUP: u32 = 16; // must match @workgroup_size in sim.wgsl
const WAKE_WORKGROUP: u32 = 8; // must match @workgroup_size of `wake`

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
    heat: wgpu::ComputePipeline,
    moves: wgpu::ComputePipeline,
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
        // Ask for the adapter's own limits, so big grids (4K) fit in one buffer.
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("sand"),
                required_limits: adapter.limits(),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sim.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sim.wgsl").into()),
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
        let heat = pipeline("heat");
        let moves = pipeline("move_blocks");
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

        let mut sim = GpuSim {
            device,
            queue,
            heat,
            moves,
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
                let mut pass = encoder.begin_compute_pass(&Default::default());
                let cells = (self.width.div_ceil(WORKGROUP), self.height.div_ceil(WORKGROUP));
                let blocks = ((self.width / 2).div_ceil(WORKGROUP), (self.height / 2).div_ceil(WORKGROUP));
                for s in 0..batch {
                    let offset = [(s as u64 * PARAMS_STRIDE) as u32];
                    let group = &self.groups[self.current];
                    // wgpu puts a barrier between dispatches that touch the same
                    // buffers, so each dispatch sees the previous one's writes.
                    pass.set_bind_group(0, group, &offset);
                    pass.set_pipeline(&self.heat);
                    pass.dispatch_workgroups(cells.0, cells.1, 1);
                    pass.set_pipeline(&self.moves);
                    pass.dispatch_workgroups(blocks.0, blocks.1, 1);
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
