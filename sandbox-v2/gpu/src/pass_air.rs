//! Persistent air pass: advection, pressure projection and buoyancy.
//! Rule defined by [`crate::air`].

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::air::{
    is_solid, AIR_DRAG, AIR_SWEEPS, AMBIENT_K, BUOYANCY_K, MAX_AIR_CPS, MAX_TEMP_EXCESS,
};
use crate::materials::WALL;
use crate::{Gpu, GridState};

const _: () = assert!(AIR_SWEEPS % 2 == 0, "Jacobi result must land in p_a");

const ENTRY_POINTS: [&str; 7] = [
    "stage_force",
    "stage_advect_vel",
    "stage_div",
    "stage_jacobi",
    "stage_project",
    "stage_advect_vapor",
    "stage_commit",
];

/// Per-grid air buffers and bind groups.
struct Cached {
    grid_id: u64,
    vel_a: wgpu::Buffer,
    /// `groups[cur][swap]`: `cur` = grid's current buffer index; `swap` = 1
    /// exchanges the two pressure buffers (odd Jacobi sweeps).
    groups: [[wgpu::BindGroup; 2]; 2],
}

/// Persistent air pass. Compiles seven compute stages once; owns the uniform
/// buffer and, per grid, the air-velocity and scratch buffers.
///
/// Air velocity is a separate `vec2<f32>` buffer (cells/s, wind excluded),
/// never `CellRaw.vx/vy`. Vapor and pressure live in the cells.
pub struct AirPass {
    pipelines: Vec<wgpu::ComputePipeline>,
    layout: wgpu::BindGroupLayout,
    ubo: wgpu::Buffer,
    sanitized: wgpu::Buffer,
    cache: Mutex<Option<Cached>>,
    dispatches: AtomicU64,
}

impl AirPass {
    /// Compiles the air pipelines.
    pub fn new(gpu: &Gpu) -> AirPass {
        let solids: Vec<String> = (0..=WALL)
            .filter(|&m| is_solid(m))
            .map(|m| format!("m == {m}u"))
            .collect();
        let header = format!(
            "const WALL: u32 = {WALL}u;\nconst MAX_AIR_CPS: f32 = {MAX_AIR_CPS:?};\n\
             const AMBIENT_K: f32 = {AMBIENT_K:?};\nconst BUOYANCY_K: f32 = {BUOYANCY_K:?};\n\
             const AIR_DRAG: f32 = {AIR_DRAG:?};\nconst MAX_TEMP_EXCESS: f32 = {MAX_TEMP_EXCESS:?};\n\
             fn is_solid(m: u32) -> bool {{ return {}; }}\n",
            solids.join(" || ")
        );
        let src = [
            header.as_str(),
            include_str!("shaders/common.wgsl"),
            include_str!("shaders/air.wgsl"),
        ]
        .join("\n");
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("air"),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            });
        let storage = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let mut entries = vec![wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }];
        entries.extend((1..=8).map(storage));
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("air_layout"),
                entries: &entries,
            });
        let pl = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("air_pl"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let pipelines = ENTRY_POINTS
            .iter()
            .map(|name| {
                gpu.device
                    .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: Some(name),
                        layout: Some(&pl),
                        module: &module,
                        entry_point: Some(name),
                        compilation_options: Default::default(),
                        cache: None,
                    })
            })
            .collect();
        let ubo = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("air_params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sanitized = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("air_sanitized"),
            size: 4,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        AirPass {
            pipelines,
            layout,
            ubo,
            sanitized,
            cache: Mutex::new(None),
            dispatches: AtomicU64::new(0),
        }
    }

    fn build(&self, gpu: &Gpu, grid: &GridState) -> Cached {
        let n = u64::from(grid.width()) * u64::from(grid.height());
        let make = |label, bytes: u64| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: n * bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        };
        let vel_a = make("air_vel_a", 8);
        let vel_b = make("air_vel_b", 8);
        let divg = make("air_div", 4);
        let p = [make("air_p_a", 4), make("air_p_b", 4)];
        let vtmp = make("air_vtmp", 4);
        let group = |cur: usize, swap: usize| {
            fn e(binding: u32, b: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
                wgpu::BindGroupEntry {
                    binding,
                    resource: b.as_entire_binding(),
                }
            }
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("air_bind"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.ubo.as_entire_binding(),
                    },
                    e(1, grid.buffer(cur)),
                    e(2, &vel_a),
                    e(3, &vel_b),
                    e(4, &divg),
                    e(5, &p[swap]),
                    e(6, &p[1 - swap]),
                    e(7, &vtmp),
                    e(8, &self.sanitized),
                ],
            })
        };
        let groups = [[group(0, 0), group(0, 1)], [group(1, 0), group(1, 1)]];
        Cached {
            grid_id: grid.id(),
            vel_a,
            groups,
        }
    }

    /// Runs one air tick of `dt` seconds with ambient `wind` (cells/s) on
    /// `grid`'s current buffer, in one encoder. Updates `vapor` and
    /// `pressure` of the cells in place; velocity stays in this pass.
    pub fn dispatch(&self, gpu: &Gpu, grid: &GridState, dt: f32, wind: [f32; 2]) {
        let (w, h) = (grid.width(), grid.height());
        let mut cache = self.cache.lock().expect("air cache poisoned");
        if cache.as_ref().is_none_or(|c| c.grid_id != grid.id()) {
            *cache = Some(self.build(gpu, grid));
        }
        let groups = &cache.as_ref().expect("cache just filled").groups[grid.current_index()];
        let mut params = [0u8; 32];
        params[0..4].copy_from_slice(&w.to_le_bytes());
        params[4..8].copy_from_slice(&h.to_le_bytes());
        params[8..12].copy_from_slice(&dt.to_le_bytes());
        params[12..16].copy_from_slice(&wind[0].to_le_bytes());
        params[16..20].copy_from_slice(&wind[1].to_le_bytes());
        gpu.queue.write_buffer(&self.ubo, 0, &params);

        let mut stages: Vec<(usize, usize)> = vec![(0, 0), (1, 0), (2, 0)];
        stages.extend((0..AIR_SWEEPS as usize).map(|s| (3, s % 2)));
        stages.extend([(4, 0), (5, 0), (6, 0)]);
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            for &(stage, swap) in &stages {
                pass.set_pipeline(&self.pipelines[stage]);
                pass.set_bind_group(0, &groups[swap], &[]);
                pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
            }
        }
        gpu.queue.submit(Some(enc.finish()));
        self.dispatches
            .fetch_add(stages.len() as u64, Ordering::Relaxed);
    }

    /// Total compute dispatches issued so far (a tick always issues
    /// `AIR_SWEEPS + 6`, whatever the grid contains).
    pub fn dispatch_count(&self) -> u64 {
        self.dispatches.load(Ordering::Relaxed)
    }

    /// Number of non-finite vapor, pressure or velocity values zeroed so far
    /// (debug counter; blocking readback, intended for tests).
    pub fn sanitized_count(&self, gpu: &Gpu) -> u32 {
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("air_sanitized_readback"),
            size: 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(&self.sanitized, 0, &staging, 0, 4);
        gpu.queue.submit(Some(enc.finish()));
        staging
            .slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.expect("map sanitized"));
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll sanitized");
        let view = staging.slice(..).get_mapped_range().expect("mapped range");
        let out = bytemuck::cast_slice::<u8, u32>(&view)[0];
        drop(view);
        staging.unmap();
        out
    }

    /// Copies the air velocity (cells/s, wind excluded) of `grid` back to the
    /// CPU (blocking). Zeros if the pass has not run on `grid` yet.
    pub fn read_velocity(&self, gpu: &Gpu, grid: &GridState) -> Vec<[f32; 2]> {
        let mut cache = self.cache.lock().expect("air cache poisoned");
        if cache.as_ref().is_none_or(|c| c.grid_id != grid.id()) {
            *cache = Some(self.build(gpu, grid));
        }
        let vel = &cache.as_ref().expect("cache just filled").vel_a;
        let size = vel.size();
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("air_readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(vel, 0, &staging, 0, size);
        gpu.queue.submit(Some(enc.finish()));
        staging
            .slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.expect("map air readback"));
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll air readback");
        let view = staging.slice(..).get_mapped_range().expect("mapped range");
        let out = bytemuck::cast_slice::<u8, [f32; 2]>(&view).to_vec();
        drop(view);
        staging.unmap();
        out
    }
}
