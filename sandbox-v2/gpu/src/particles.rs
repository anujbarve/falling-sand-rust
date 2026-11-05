//! GPU particle system: up to [`Particles::MAX`] particles in one storage
//! buffer, stepped by a persistent compute pass. Rule defined by
//! [`crate::reference_particles`].
//!
//! Bindings (one stage, `step`): 0 uniform params (32 bytes), 1 particles
//! (read-write storage), 2 grid cells (read-only storage), 3 sanitize
//! counter (read-write storage). That is 3 storage buffers, under the limit of 8.
//!
//! The ring cursor (head, count) lives on the CPU because spawning is a CPU
//! upload; the count is mirrored to the GPU in the params uniform each step.

use crate::air::is_solid;
use crate::layout::ParticleRaw;
use crate::materials::WALL;
use crate::reference_particles::{
    Ring, StepConsts, FP_BITS, FP_ONE, MAX_SIDE, MAX_SPEED_CPS, MAX_SPEED_FP, REST_DEN, REST_NUM,
    REST_VY_FP,
};
use crate::{Gpu, GridState};

const PART_BYTES: u64 = std::mem::size_of::<ParticleRaw>() as u64;
const WORKGROUP: u32 = 256;

/// Compiled step pipeline, its uniform and counter, and bind groups cached
/// for one grid (`groups[grid current index]`).
struct ParticlePass {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    ubo: wgpu::Buffer,
    counter: wgpu::Buffer,
    cache: Option<(u64, [wgpu::BindGroup; 2])>,
}

fn header() -> String {
    let solids: Vec<String> = (0..=WALL)
        .filter(|&m| is_solid(m))
        .map(|m| format!("m == {m}u"))
        .collect();
    format!(
        "const WALL: u32 = {WALL}u;\nconst FP_ONE: i32 = {FP_ONE};\nconst FP_BITS: u32 = {FP_BITS}u;\n\
         const MAX_SPEED_CPS: f32 = {MAX_SPEED_CPS:?};\nconst MAX_SPEED_FP: i32 = {MAX_SPEED_FP};\n\
         const REST_NUM: i32 = {REST_NUM};\nconst REST_DEN: i32 = {REST_DEN};\n\
         const REST_VY_FP: i32 = {REST_VY_FP};\n\
         fn is_solid(m: u32) -> bool {{ return {}; }}\n",
        solids.join(" || ")
    )
}

fn entry(binding: u32, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

impl ParticlePass {
    fn new(gpu: &Gpu) -> ParticlePass {
        let src = [
            header().as_str(),
            include_str!("shaders/common.wgsl"),
            include_str!("shaders/particles.wgsl"),
        ]
        .join("\n");
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("particles"),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            });
        use wgpu::BufferBindingType::{Storage, Uniform};
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("particles"),
                entries: &[
                    entry(0, Uniform),
                    entry(1, Storage { read_only: false }),
                    entry(2, Storage { read_only: true }),
                    entry(3, Storage { read_only: false }),
                ],
            });
        let pl = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("particles"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("particles_step"),
                layout: Some(&pl),
                module: &module,
                entry_point: Some("step"),
                compilation_options: Default::default(),
                cache: None,
            });
        let ubo = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particle_params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let counter = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particle_sanitized"),
            size: 4,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ParticlePass {
            pipeline,
            layout,
            ubo,
            counter,
            cache: None,
        }
    }

    /// Builds the bind groups for `grid` unless already cached.
    fn ensure_groups(&mut self, gpu: &Gpu, parts: &wgpu::Buffer, grid: &GridState) {
        if self.cache.as_ref().is_none_or(|(id, _)| *id != grid.id()) {
            let make = |cur: usize| {
                fn e(binding: u32, b: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
                    wgpu::BindGroupEntry {
                        binding,
                        resource: b.as_entire_binding(),
                    }
                }
                gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("particles_bind"),
                    layout: &self.layout,
                    entries: &[
                        e(0, &self.ubo),
                        e(1, parts),
                        e(2, grid.buffer(cur)),
                        e(3, &self.counter),
                    ],
                })
            };
            self.cache = Some((grid.id(), [make(0), make(1)]));
        }
    }
}

fn try_read_buffer(gpu: &Gpu, src: &wgpu::Buffer, size: u64) -> Option<Vec<u8>> {
    let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("particle_readback"),
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    enc.copy_buffer_to_buffer(src, 0, &staging, 0, size);
    gpu.queue.submit(Some(enc.finish()));
    let ok = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let ok2 = ok.clone();
    staging.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        if r.is_ok() {
            ok2.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    });
    if gpu
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .is_err()
    {
        return None;
    }
    if !ok.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    let view = staging.slice(..).get_mapped_range().ok()?;
    let out = view.to_vec();
    drop(view);
    staging.unmap();
    Some(out)
}

fn read_buffer(gpu: &Gpu, src: &wgpu::Buffer, size: u64) -> Vec<u8> {
    try_read_buffer(gpu, src, size).expect("read particle buffer failed")
}

/// A GPU ring buffer of particles plus its persistent step pass.
pub struct Particles {
    cap: u32,
    buf: wgpu::Buffer,
    ring: Ring,
    pass: ParticlePass,
}

impl Particles {
    /// Largest supported capacity (one million particles, 32 MB).
    pub const MAX: u32 = 1_000_000;

    /// Allocates `cap` slots and compiles the step pass once.
    ///
    /// # Panics
    /// If `cap` is 0 or above [`Particles::MAX`].
    pub fn new(gpu: &Gpu, cap: u32) -> Particles {
        assert!(
            (1..=Self::MAX).contains(&cap),
            "particle cap {cap} not in 1..={}",
            Self::MAX
        );
        let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("particles"),
            size: u64::from(cap) * PART_BYTES,
            // VERTEX: the renderer draws particles as instanced sprites.
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Particles {
            cap,
            buf,
            ring: Ring::new(cap),
            pass: ParticlePass::new(gpu),
        }
    }

    /// Slot capacity.
    pub fn cap(&self) -> u32 {
        self.cap
    }

    /// Occupied slots (live or dead), at most [`Particles::cap`].
    pub fn count(&self) -> u32 {
        self.ring.count()
    }

    /// The particle storage buffer (`count` valid [`ParticleRaw`] entries).
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buf
    }

    /// Uploads `batch` at the ring head. Beyond the cap the oldest slots are
    /// overwritten (only the newest `cap` entries of a larger batch are kept).
    pub fn spawn(&mut self, gpu: &Gpu, batch: &[ParticleRaw]) {
        for c in self.ring.push(batch.len()) {
            gpu.queue.write_buffer(
                &self.buf,
                u64::from(c.dst) * PART_BYTES,
                bytemuck::cast_slice(&batch[c.src..c.src + c.len]),
            );
        }
    }

    /// Records a step of every occupied slot by `dt` seconds against
    /// `grid`'s current buffer into `pass` (one dispatch, no submit).
    ///
    /// # Panics
    /// Unless `dt = 1/n` s with `n >= 60`, or if a grid side exceeds
    /// [`MAX_SIDE`](crate::reference_particles::MAX_SIDE).
    pub fn record_step(
        &mut self,
        gpu: &Gpu,
        pass: &mut wgpu::ComputePass<'_>,
        grid: &GridState,
        dt: f32,
    ) {
        let k = StepConsts::new(dt);
        let (w, h) = (grid.width(), grid.height());
        assert!(w <= MAX_SIDE && h <= MAX_SIDE, "grid side above {MAX_SIDE}");
        let count = self.ring.count();
        let mut params = [0u32; 8];
        params[..7].copy_from_slice(&[
            count,
            w,
            h,
            k.hz as u32,
            k.grav_fp as u32,
            k.dt.to_bits(),
            k.fric_fp as u32,
        ]);
        gpu.queue
            .write_buffer(&self.pass.ubo, 0, bytemuck::cast_slice(&params));
        self.pass.ensure_groups(gpu, &self.buf, grid);
        let group = &self.pass.cache.as_ref().expect("cache just filled").1[grid.current_index()];
        if count > 0 {
            pass.set_pipeline(&self.pass.pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.dispatch_workgroups(count.div_ceil(WORKGROUP), 1, 1);
        }
    }

    /// Advances every occupied slot by `dt` seconds against `grid`'s current
    /// buffer (one dispatch, one submit).
    ///
    /// # Panics
    /// As [`Particles::record_step`].
    pub fn step(&mut self, gpu: &Gpu, grid: &GridState, dt: f32) {
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        self.record_step(
            gpu,
            &mut enc.begin_compute_pass(&Default::default()),
            grid,
            dt,
        );
        gpu.queue.submit(Some(enc.finish()));
    }

    /// Copies the occupied slots back in slot order (blocking).
    /// Returns `None` if mapping or device polling fails (e.g. device lost).
    pub fn try_readback(&self, gpu: &Gpu) -> Option<Vec<ParticleRaw>> {
        let size = u64::from(self.count()) * PART_BYTES;
        if size == 0 {
            return Some(Vec::new());
        }
        let bytes = try_read_buffer(gpu, &self.buf, size)?;
        Some(bytemuck::pod_collect_to_vec(&bytes))
    }

    /// Copies the occupied slots back in slot order (blocking).
    pub fn readback(&self, gpu: &Gpu) -> Vec<ParticleRaw> {
        let size = u64::from(self.count()) * PART_BYTES;
        if size == 0 {
            return Vec::new();
        }
        bytemuck::pod_collect_to_vec(&read_buffer(gpu, &self.buf, size))
    }

    /// Particles zeroed for non-finite fields so far (debug counter;
    /// blocking readback, intended for tests).
    pub fn try_sanitized_count(&self, gpu: &Gpu) -> Option<u32> {
        let bytes = try_read_buffer(gpu, &self.pass.counter, 4)?;
        Some(bytemuck::pod_read_unaligned(&bytes))
    }

    /// Particles zeroed for non-finite fields so far (debug counter;
    /// blocking readback, intended for tests).
    pub fn sanitized_count(&self, gpu: &Gpu) -> u32 {
        bytemuck::pod_read_unaligned(&read_buffer(gpu, &self.pass.counter, 4))
    }
}
