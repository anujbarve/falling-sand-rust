//! Shared dispatch machinery for the row-parity passes.

use wgpu::util::DeviceExt;

use crate::{Gpu, GridState};

/// A compute pipeline for one row-parity rule (two phases per tick).
pub(crate) struct RowPass {
    pipeline: wgpu::ComputePipeline,
}

impl RowPass {
    /// Compiles `common + rules + rule_src` into a pipeline.
    pub(crate) fn new(gpu: &Gpu, label: &str, rule_src: &str) -> RowPass {
        let src = [
            include_str!("shaders/common.wgsl"),
            include_str!("shaders/rules.wgsl"),
            rule_src,
        ]
        .join("\n");
        let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
        let pipeline = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(label),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        RowPass { pipeline }
    }

    /// Runs phase 0 then phase 1, swapping `grid` after each phase.
    pub(crate) fn dispatch(&self, gpu: &Gpu, grid: &mut GridState, tick: u32, seed: u32) {
        let (w, h) = (grid.width(), grid.height());
        for phase in 0..2u32 {
            let params: [u32; 8] = [w, h, tick, seed, phase, 0, 0, 0];
            let ubo = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("row_params"),
                contents: bytemuck::cast_slice(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("row_bind"),
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: ubo.as_entire_binding() },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: grid.current().as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: grid.other().as_entire_binding(),
                    },
                ],
            });
            let mut enc = gpu.device.create_command_encoder(&Default::default());
            {
                let mut pass = enc.begin_compute_pass(&Default::default());
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &bind, &[]);
                pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
            }
            gpu.queue.submit(Some(enc.finish()));
            grid.swap();
        }
    }
}
