//! Shared persistent machinery for the thermal, fire and lava passes.
//!
//! One bind group layout for every stage: binding 0 = uniform params
//! (`w, h, tick, seed, phase` + padding, 32 bytes), binding 1 = source cells
//! (read-only storage), binding 2 = destination cells (read-write storage),
//! binding 3 = a 4-byte debug counter (read-write storage), binding 4 = the
//! grid's event counters (read-write storage). That is 4 storage buffers per
//! stage, under the default limit of 8.

use std::sync::Mutex;

use crate::{Gpu, GridState};

/// Byte stride between the two parameter slots (the guaranteed minimum
/// `min_uniform_buffer_offset_alignment`).
const SLOT: u64 = 256;

/// Which buffer a stage reads and writes.
#[derive(Clone, Copy)]
pub(crate) enum Flow {
    /// src = current buffer, dst = other buffer.
    Forward,
    /// src = other buffer, dst = current buffer (also used by in-place
    /// stages, which only touch `dst`).
    Back,
}

/// One dispatch: entry point index, buffer flow and parameter slot (the
/// slot number is passed to the shader as `phase`).
#[derive(Clone, Copy)]
pub(crate) struct Stage {
    pub(crate) entry: usize,
    pub(crate) flow: Flow,
    pub(crate) slot: usize,
}

/// `groups[cur][flow][slot]` for one grid.
struct Cached {
    grid_id: u64,
    groups: [[[wgpu::BindGroup; 2]; 2]; 2],
}

/// Compiled pipelines (one per entry point) over the shared layout, plus
/// the uniform buffer, debug counter and cached bind groups.
pub(crate) struct CellPass {
    pipelines: Vec<wgpu::ComputePipeline>,
    layout: wgpu::BindGroupLayout,
    ubo: wgpu::Buffer,
    counter: wgpu::Buffer,
    cache: Mutex<Option<Cached>>,
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

impl CellPass {
    /// Compiles `source` once with one pipeline per name in `entries`.
    pub(crate) fn new(gpu: &Gpu, label: &str, source: String, entries: &[&str]) -> CellPass {
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
        use wgpu::BufferBindingType::{Storage, Uniform};
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries: &[
                    entry(0, Uniform),
                    entry(1, Storage { read_only: true }),
                    entry(2, Storage { read_only: false }),
                    entry(3, Storage { read_only: false }),
                    entry(4, Storage { read_only: false }),
                ],
            });
        let pl = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let pipelines = entries
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
            label: Some("cell_params"),
            size: 2 * SLOT,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let counter = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cell_counter"),
            size: 4,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        CellPass {
            pipelines,
            layout,
            ubo,
            counter,
            cache: Mutex::new(None),
        }
    }

    fn build(&self, gpu: &Gpu, grid: &GridState) -> Cached {
        let make = |cur: usize, flow: usize, slot: usize| {
            // flow 0 (Forward): cur -> other; flow 1 (Back): other -> cur.
            let (src, dst) = if flow == 0 {
                (cur, 1 - cur)
            } else {
                (1 - cur, cur)
            };
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cell_bind"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.ubo,
                            offset: slot as u64 * SLOT,
                            size: wgpu::BufferSize::new(32),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: grid.buffer(src).as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: grid.buffer(dst).as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.counter.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: grid.events().as_entire_binding(),
                    },
                ],
            })
        };
        let per_cur = |cur| {
            [
                [make(cur, 0, 0), make(cur, 0, 1)],
                [make(cur, 1, 0), make(cur, 1, 1)],
            ]
        };
        Cached {
            grid_id: grid.id(),
            groups: [per_cur(0), per_cur(1)],
        }
    }

    /// Records `stages` into `pass` (no submit). Buffer roles follow each
    /// stage's [`Flow`] relative to `grid`'s current buffer at record time.
    pub(crate) fn record(
        &self,
        gpu: &Gpu,
        pass: &mut wgpu::ComputePass<'_>,
        grid: &GridState,
        tick: u32,
        seed: u32,
        stages: &[Stage],
    ) {
        let (w, h) = (grid.width(), grid.height());
        let mut cache = self.cache.lock().expect("cell pass cache poisoned");
        if cache.as_ref().is_none_or(|c| c.grid_id != grid.id()) {
            *cache = Some(self.build(gpu, grid));
        }
        let groups = &cache.as_ref().expect("cache just filled").groups[grid.current_index()];
        for slot in 0..2u32 {
            let params: [u32; 8] = [w, h, tick, seed, slot, 0, 0, 0];
            gpu.queue.write_buffer(
                &self.ubo,
                u64::from(slot) * SLOT,
                bytemuck::cast_slice(&params),
            );
        }
        for st in stages {
            let flow = match st.flow {
                Flow::Forward => 0,
                Flow::Back => 1,
            };
            pass.set_pipeline(&self.pipelines[st.entry]);
            pass.set_bind_group(0, &groups[flow][st.slot], &[]);
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }
    }

    /// Reads the debug counter (blocking; intended for tests).
    pub(crate) fn counter(&self, gpu: &Gpu) -> u32 {
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cell_counter_readback"),
            size: 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(&self.counter, 0, &staging, 0, 4);
        gpu.queue.submit(Some(enc.finish()));
        staging
            .slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.expect("map counter"));
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll counter");
        let view = staging.slice(..).get_mapped_range().expect("mapped range");
        let out = bytemuck::cast_slice::<u8, u32>(&view)[0];
        drop(view);
        staging.unmap();
        out
    }
}
