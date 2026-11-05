//! Shared persistent dispatch machinery for the row-parity passes.
//!
//! Bindings: 0 uniform params, 1 source cells (read), 2 destination cells
//! (read-write), 3 event counters (read-write atomics). 3 storage buffers.

use std::sync::Mutex;

use crate::reference::{EMPTY, WALL};
use crate::units::{cps_to_fp, FP_ONE, GRAVITY_FP, MAX_STEP};
use crate::{Gpu, GridState};

/// Byte stride between the two phase slots in the uniform buffer (the
/// guaranteed minimum `min_uniform_buffer_offset_alignment`).
const SLOT: u64 = 256;

/// Bind groups for one grid: `groups[cur][phase]` where `cur` is the index of
/// the grid's current buffer when the dispatch starts.
struct Cached {
    grid_id: u64,
    groups: [[wgpu::BindGroup; 2]; 2],
}

/// A compiled compute pipeline for one row-parity rule (two phases per tick),
/// with its own uniform buffer and bind groups.
pub(crate) struct RowPass {
    pipeline: wgpu::ComputePipeline,
    ubo: wgpu::Buffer,
    cache: Mutex<Option<Cached>>,
}

impl RowPass {
    /// Compiles the shared rule for movers of material id `mover`, with
    /// `terminal_cps` as the speed limit and `sideways` enabling liquid spreading.
    pub(crate) fn new(
        gpu: &Gpu,
        label: &str,
        mover: u32,
        sideways: bool,
        terminal_cps: f32,
    ) -> RowPass {
        let terminal_fp = cps_to_fp(terminal_cps);
        assert!(
            terminal_fp <= MAX_STEP as i32 * FP_ONE,
            "terminal speed {terminal_cps} cells/s exceeds MAX_STEP per tick"
        );
        let header = format!(
            "const FP_ONE: i32 = {FP_ONE};\nconst MAX_STEP: i32 = {MAX_STEP};\n\
             const GRAVITY_FP: i32 = {GRAVITY_FP};\nconst TERMINAL_FP: i32 = {terminal_fp};\n\
             const MOVER: u32 = {mover}u;\nconst EMPTY: u32 = {EMPTY}u;\nconst WALL: u32 = {WALL}u;\nconst SIDEWAYS: bool = {sideways};\n"
        );
        let src = [
            header.as_str(),
            include_str!("shaders/common.wgsl"),
            include_str!("shaders/hash.wgsl"),
            include_str!("shaders/rules.wgsl"),
        ]
        .join("\n");
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            });
        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: None,
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
        let ubo = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("row_params"),
            size: 2 * SLOT,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        RowPass {
            pipeline,
            ubo,
            cache: Mutex::new(None),
        }
    }

    fn bind_groups(&self, gpu: &Gpu, grid: &GridState) -> [[wgpu::BindGroup; 2]; 2] {
        let layout = self.pipeline.get_bind_group_layout(0);
        let make = |cur: usize, phase: usize| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("row_bind"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.ubo,
                            offset: phase as u64 * SLOT,
                            size: wgpu::BufferSize::new(32),
                        }),
                    },
                    // Phase 0 reads the current buffer, phase 1 reads the other.
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: grid.buffer(cur ^ phase).as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: grid.buffer(cur ^ phase ^ 1).as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: grid.events().as_entire_binding(),
                    },
                ],
            })
        };
        [[make(0, 0), make(0, 1)], [make(1, 0), make(1, 1)]]
    }

    /// Records phase 0 (current -> other) then phase 1 (other -> current)
    /// into `pass`. The result is back in `grid`'s current buffer; no swap.
    pub(crate) fn record(
        &self,
        gpu: &Gpu,
        pass: &mut wgpu::ComputePass<'_>,
        grid: &GridState,
        tick: u32,
        seed: u32,
    ) {
        let (w, h) = (grid.width(), grid.height());
        let mut cache = self.cache.lock().expect("pass cache poisoned");
        if cache.as_ref().is_none_or(|c| c.grid_id != grid.id()) {
            *cache = Some(Cached {
                grid_id: grid.id(),
                groups: self.bind_groups(gpu, grid),
            });
        }
        let groups = &cache.as_ref().expect("cache just filled").groups[grid.current_index()];
        for phase in 0..2u32 {
            let params: [u32; 8] = [w, h, tick, seed, phase, 0, 0, 0];
            gpu.queue.write_buffer(
                &self.ubo,
                u64::from(phase) * SLOT,
                bytemuck::cast_slice(&params),
            );
        }
        pass.set_pipeline(&self.pipeline);
        for group in groups {
            pass.set_bind_group(0, group, &[]);
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }
    }

    /// [`RowPass::record`] in its own encoder, then one submit.
    pub(crate) fn dispatch(&self, gpu: &Gpu, grid: &GridState, tick: u32, seed: u32) {
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        self.record(
            gpu,
            &mut enc.begin_compute_pass(&Default::default()),
            grid,
            tick,
            seed,
        );
        gpu.queue.submit(Some(enc.finish()));
    }
}
