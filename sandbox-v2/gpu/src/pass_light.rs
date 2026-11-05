//! Light field pass (`shaders/light.wgsl`): blackbody emission per cell, a
//! jump flood ([`LIGHT_STEPS`]) that finds each cell's dominant emitter
//! within [`LIGHT_RADIUS`](crate::render::LIGHT_RADIUS), then a resolve that
//! applies the distance falloff and a ray-marched occluder test (solid cells
//! cast shadows).
//!
//! Bindings per stage (storage buffers / storage textures, limit 8):
//! `emit` 1/2 (cells; emission and seed), `jump` 0/1, `resolve` 1/1, plus
//! uniforms and sampled textures.

use crate::render::{
    buffer_entry, compute_pipeline, storage_tex_entry, texture, texture_entry, LIGHT_STEPS,
};
use crate::{Gpu, GridState};

const _: () = assert!(
    LIGHT_STEPS.len().is_multiple_of(2),
    "jump flood must end in source texture 0"
);

/// Compiled pipelines, layouts and the per-pass jump-step uniforms.
pub(crate) struct LightPass {
    emit: wgpu::ComputePipeline,
    jump: wgpu::ComputePipeline,
    resolve: wgpu::ComputePipeline,
    emit_layout: wgpu::BindGroupLayout,
    jump_layout: wgpu::BindGroupLayout,
    resolve_layout: wgpu::BindGroupLayout,
    steps: Vec<wgpu::Buffer>,
}

/// Grid-sized light textures and bind groups. `light` holds the result
/// (rgb = light, a = 1 for solid cells).
pub(crate) struct LightTargets {
    pub(crate) light: wgpu::TextureView,
    emit: [wgpu::BindGroup; 2],
    jumps: Vec<wgpu::BindGroup>,
    resolve: [wgpu::BindGroup; 2],
}

fn res(binding: u32, resource: wgpu::BindingResource<'_>) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource }
}

impl LightPass {
    /// Compiles the three pipelines from the light module.
    pub(crate) fn new(gpu: &Gpu, module: &wgpu::ShaderModule) -> LightPass {
        use wgpu::BufferBindingType::{Storage, Uniform};
        let cs = wgpu::ShaderStages::COMPUTE;
        let d = &gpu.device;
        let layout = |label, entries: &[wgpu::BindGroupLayoutEntry]| {
            d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries,
            })
        };
        let cells = buffer_entry(1, cs, Storage { read_only: true });
        let emit_layout = layout(
            "light_emit",
            &[
                buffer_entry(0, cs, Uniform),
                cells,
                storage_tex_entry(2),
                storage_tex_entry(5),
            ],
        );
        let jump_layout = layout(
            "light_jump",
            &[
                buffer_entry(0, cs, Uniform),
                storage_tex_entry(2),
                texture_entry(3, cs),
                texture_entry(4, cs),
                buffer_entry(6, cs, Uniform),
            ],
        );
        let resolve_layout = layout(
            "light_resolve",
            &[
                buffer_entry(0, cs, Uniform),
                cells,
                storage_tex_entry(2),
                texture_entry(3, cs),
                texture_entry(4, cs),
            ],
        );
        let steps = LIGHT_STEPS
            .iter()
            .map(|&s| {
                let b = d.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("light_step"),
                    size: 16,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                gpu.queue
                    .write_buffer(&b, 0, bytemuck::cast_slice(&[s as i32, 0, 0, 0]));
                b
            })
            .collect();
        LightPass {
            emit: compute_pipeline(gpu, &emit_layout, module, "emit"),
            jump: compute_pipeline(gpu, &jump_layout, module, "jump"),
            resolve: compute_pipeline(gpu, &resolve_layout, module, "resolve"),
            emit_layout,
            jump_layout,
            resolve_layout,
            steps,
        }
    }

    /// Allocates the light textures for `grid` and builds every bind group
    /// (emit and resolve for both grid buffers).
    pub(crate) fn targets(&self, gpu: &Gpu, view: &wgpu::Buffer, grid: &GridState) -> LightTargets {
        let (w, h) = (grid.width(), grid.height());
        let st = wgpu::TextureUsages::STORAGE_BINDING;
        let emis = texture(gpu, "light_emission", w, h, st);
        let src = [
            texture(gpu, "light_source_0", w, h, st),
            texture(gpu, "light_source_1", w, h, st),
        ];
        let light = texture(gpu, "light", w, h, st);
        use wgpu::BindingResource::TextureView as Tv;
        let group =
            |label, layout: &wgpu::BindGroupLayout, entries: &[wgpu::BindGroupEntry<'_>]| {
                gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(label),
                    layout,
                    entries,
                })
            };
        let emit = [0, 1].map(|cur| {
            group(
                "light_emit",
                &self.emit_layout,
                &[
                    res(0, view.as_entire_binding()),
                    res(1, grid.buffer(cur).as_entire_binding()),
                    res(2, Tv(&emis)),
                    res(5, Tv(&src[0])),
                ],
            )
        });
        let jumps = self
            .steps
            .iter()
            .enumerate()
            .map(|(i, step)| {
                group(
                    "light_jump",
                    &self.jump_layout,
                    &[
                        res(0, view.as_entire_binding()),
                        res(2, Tv(&src[(i + 1) % 2])),
                        res(3, Tv(&emis)),
                        res(4, Tv(&src[i % 2])),
                        res(6, step.as_entire_binding()),
                    ],
                )
            })
            .collect();
        let resolve = [0, 1].map(|cur| {
            group(
                "light_resolve",
                &self.resolve_layout,
                &[
                    res(0, view.as_entire_binding()),
                    res(1, grid.buffer(cur).as_entire_binding()),
                    res(2, Tv(&light)),
                    res(3, Tv(&emis)),
                    res(4, Tv(&src[0])),
                ],
            )
        });
        LightTargets {
            light,
            emit,
            jumps,
            resolve,
        }
    }

    /// Records emit, the jump flood and the resolve for grid buffer `cur`.
    pub(crate) fn record(
        &self,
        pass: &mut wgpu::ComputePass<'_>,
        t: &LightTargets,
        cur: usize,
        w: u32,
        h: u32,
    ) {
        let (gx, gy) = (w.div_ceil(8), h.div_ceil(8));
        pass.set_pipeline(&self.emit);
        pass.set_bind_group(0, &t.emit[cur], &[]);
        pass.dispatch_workgroups(gx, gy, 1);
        pass.set_pipeline(&self.jump);
        for g in &t.jumps {
            pass.set_bind_group(0, g, &[]);
            pass.dispatch_workgroups(gx, gy, 1);
        }
        pass.set_pipeline(&self.resolve);
        pass.set_bind_group(0, &t.resolve[cur], &[]);
        pass.dispatch_workgroups(gx, gy, 1);
    }
}
