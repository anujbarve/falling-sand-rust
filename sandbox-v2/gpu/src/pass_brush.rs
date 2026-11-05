//! Persistent brush pass: paints discs of a material into Empty cells (the
//! v1 brush rule: painting never deletes anything), in place on the grid's
//! current buffer. Used by mouse painting and by scene emitters (vents).
//!
//! Bindings: 0 uniform params, 1 strokes (read-only storage), 2 cells
//! (read-write storage). 2 storage buffers.
//!
//! Stroke values are sanitised on the CPU before upload: a non-finite
//! `temp` becomes ambient, a non-finite `fuel` becomes 0, and each is
//! counted ([`BrushPass::sanitized`]).

use std::sync::Mutex;

use bytemuck::{Pod, Zeroable};

use crate::air::AMBIENT_K;
use crate::materials::Material;
use crate::{Gpu, GridState};

/// Most strokes applied in one recorded brush dispatch; extras are dropped.
pub const MAX_STROKES: usize = 64;

/// One disc to paint: centre `(cx, cy)` in cells, radius `r` in cells.
/// Matches `struct Stroke` in `shaders/brush.wgsl` (32 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Stroke {
    /// Centre column (may lie outside the grid).
    pub cx: i32,
    /// Centre row (may lie outside the grid).
    pub cy: i32,
    /// Radius in cells (a cell is painted when `dx^2 + dy^2 <= r^2`).
    pub r: i32,
    /// Material id painted.
    pub mat: u32,
    /// Temperature of painted cells, kelvin.
    pub temp: f32,
    /// Timer value of painted cells (see `reference_thermal`).
    pub fuel: f32,
    /// Unused; keep 0.
    pub _pad: [u32; 2],
}

/// Compiled brush pipeline, its buffers and per-grid bind groups.
pub struct BrushPass {
    pipeline: wgpu::ComputePipeline,
    ubo: wgpu::Buffer,
    strokes: wgpu::Buffer,
    cache: Mutex<Option<(u64, [wgpu::BindGroup; 2])>>,
    sanitized: u32,
}

impl BrushPass {
    /// Compiles the brush pipeline.
    pub fn new(gpu: &Gpu) -> BrushPass {
        let src = [
            include_str!("shaders/common.wgsl"),
            include_str!("shaders/brush.wgsl"),
        ]
        .join("\n");
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("brush"),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            });
        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("brush"),
                layout: None,
                module: &module,
                entry_point: Some("stage_brush"),
                compilation_options: Default::default(),
                cache: None,
            });
        let buf = |label, size, usage| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        BrushPass {
            pipeline,
            ubo: buf("brush_params", 16, wgpu::BufferUsages::UNIFORM),
            strokes: buf(
                "brush_strokes",
                (MAX_STROKES * std::mem::size_of::<Stroke>()) as u64,
                wgpu::BufferUsages::STORAGE,
            ),
            cache: Mutex::new(None),
            sanitized: 0,
        }
    }

    /// Non-finite stroke values replaced so far.
    pub fn sanitized(&self) -> u32 {
        self.sanitized
    }

    /// Records one dispatch painting `strokes` (at most [`MAX_STROKES`]; the
    /// rest are dropped) into `pass`. Does nothing for an empty slice.
    pub fn record(
        &mut self,
        gpu: &Gpu,
        pass: &mut wgpu::ComputePass<'_>,
        grid: &GridState,
        strokes: &[Stroke],
    ) {
        let strokes = &strokes[..strokes.len().min(MAX_STROKES)];
        if strokes.is_empty() {
            return;
        }
        let clean: Vec<Stroke> = strokes
            .iter()
            .map(|s| {
                let mut s = *s;
                if !s.temp.is_finite() {
                    s.temp = AMBIENT_K;
                    self.sanitized += 1;
                }
                if !s.fuel.is_finite() {
                    s.fuel = 0.0;
                    self.sanitized += 1;
                }
                s
            })
            .collect();
        let (w, h) = (grid.width(), grid.height());
        let params = [w, h, clean.len() as u32, Material::Empty as u32];
        gpu.queue
            .write_buffer(&self.ubo, 0, bytemuck::cast_slice(&params));
        gpu.queue
            .write_buffer(&self.strokes, 0, bytemuck::cast_slice(&clean));
        let mut cache = self.cache.lock().expect("brush cache poisoned");
        if cache.as_ref().is_none_or(|(id, _)| *id != grid.id()) {
            let layout = self.pipeline.get_bind_group_layout(0);
            let make = |cur: usize| {
                gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("brush_bind"),
                    layout: &layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.ubo.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: self.strokes.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: grid.buffer(cur).as_entire_binding(),
                        },
                    ],
                })
            };
            *cache = Some((grid.id(), [make(0), make(1)]));
        }
        let group = &cache.as_ref().expect("cache just filled").1[grid.current_index()];
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
    }
}
