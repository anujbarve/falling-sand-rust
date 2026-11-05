//! The renderer: turns the cell grid and the particles into a lit,
//! tone-mapped image. Look rules and every constant live here; the shaders
//! receive them through a generated header.
//!
//! Per frame (one encoder, one submit, all pipelines built once in
//! [`Renderer::new`]):
//! 1. Light (`light.wgsl`, cell resolution): blackbody emission of every cell
//!    hotter than [`EMIT_MIN_K`]; a jump flood ([`LIGHT_STEPS`]) finds each
//!    cell's dominant emitter within [`LIGHT_RADIUS`]; light = emission x
//!    windowed inverse-square falloff ([`LIGHT_D0`]), zero when a solid cell
//!    lies on the straight line to the emitter (ray-marched shadow).
//! 2. Shade (`shade.wgsl`, cell resolution, HDR): albedo with hash noise, lit
//!    by `AMBIENT + LIGHT_GAIN * light`; liquid depth absorption and a
//!    surface highlight from neighbouring column heights; gases show the
//!    background, light glow in air and vapor/steam/smoke with
//!    `alpha = 1 - exp(-VAPOR_K * vapor)`; blackbody emission is added.
//! 3. Scene (`scene.wgsl`, [`scene_scale`] pixels per cell, HDR): sharpened
//!    bilinear upsample of the shaded cells (edges blend over [`EDGE_PX`]
//!    pixels), then one instanced draw of every occupied particle slot (up to
//!    [`PARTICLE_DRAW_CAP`]) as a round alpha-blended sprite.
//! 4. Bloom (`bloom.wgsl`): bright pass above [`BLOOM_THRESHOLD`] at quarter
//!    scene resolution, separable Gaussian ([`BLOOM_SIGMA`], [`BLOOM_RADIUS`]).
//! 5. Composite (`composite.wgsl`): `scene + BLOOM_STRENGTH * bloom`, times
//!    [`EXPOSURE`], ACES filmic fit (Narkowicz 2015), clamp to 0..1, sRGB
//!    encode for non-sRGB targets.
//!
//! The render is not parity-checked; `tests/render_golden.rs` is its spec.
//! It is deterministic: no atomics, no races, noise is a hash of
//! `(x, y, NOISE_SEED)`, so the same input gives the same output bytes.
//!
//! Binding budget (storage buffers / storage textures per stage, limit 8):
//! light emit 1/2, light jump 0/1, light resolve 1/1, shade 1/1, scene fill
//! 0/0, sprites 0/0 (particles
//! are a vertex buffer), bloom 0/0, composite 0/0.

use crate::air::{is_solid, AMBIENT_K};
use crate::materials::{Material, MATERIAL_COUNT, WALL};
use crate::pass_light::{LightPass, LightTargets};
use crate::{Gpu, GridState, Particles};

/// Hash seed for the per-cell colour noise.
pub const NOISE_SEED: u32 = 0x5EED_1234;
/// Sky/ambient light every cell receives (linear, 1 = full albedo).
pub const AMBIENT: f32 = 0.55;
/// Scale from the light field to illumination of surfaces.
pub const LIGHT_GAIN: f32 = 0.35;
/// Fraction of the light field scattered by air (glow around emitters).
pub const AIR_GLOW: f32 = 0.04;

/// Glow starts here: the Draper point, where hot bodies first glow dull red
/// (798 K, J. W. Draper 1847), rounded.
pub const EMIT_MIN_K: f32 = 800.0;
/// Emission is faded in (smoothstep) from [`EMIT_MIN_K`] to this.
pub const EMIT_FULL_K: f32 = 1000.0;
/// Reference temperature of the intensity law (the fire temperature).
pub const EMIT_REF_K: f32 = 1200.0;
/// Emission at [`EMIT_REF_K`]; intensity scales as `(T / EMIT_REF_K)^4`
/// (Stefan-Boltzmann), so 1500 K lava is 2.44x a 1200 K flame.
pub const EMIT_GAIN: f32 = 2.5;
/// Emission intensity cap (keeps HDR values bounded).
pub const EMIT_MAX: f32 = 32.0;
/// Blackbody colour ramp: (kelvin, linear sRGB normalised to the brightest
/// channel). From Mitchell Charity's blackbody colour table ("What colour is
/// a blackbody?", vendian.org, CIE 1931 2 deg) converted from sRGB to linear;
/// 800 K extrapolated as pure dim red. Dim red -> orange -> yellow-white.
pub const BLACKBODY: [(f32, [f32; 3]); 8] = [
    (800.0, [1.0, 0.0, 0.0]),
    (1000.0, [1.0, 0.0395, 0.0]),
    (1200.0, [1.0, 0.0865, 0.0]),
    (1500.0, [1.0, 0.153, 0.0]),
    (2000.0, [1.0, 0.254, 0.006]),
    (3000.0, [1.0, 0.456, 0.147]),
    (5000.0, [1.0, 0.776, 0.617]),
    (6500.0, [1.0, 0.947, 0.982]),
];

/// Light reach in cells: the falloff window reaches exactly 0 here.
pub const LIGHT_RADIUS: f32 = 32.0;
/// Distance (cells) at which the inverse-square part halves the light:
/// `falloff(d) = clamp(1 - (d/R)^4, 0, 1)^2 / (1 + (d/D0)^2)` (Karis 2013,
/// "Real Shading in Unreal Engine 4", windowed inverse square).
pub const LIGHT_D0: f32 = 6.0;
/// Jump-flood step sizes, one compute pass each (Rong & Tan 2006, plus a
/// final 1-step pass, "JFA+1"); 16 + 8 + ... covers [`LIGHT_RADIUS`].
pub const LIGHT_STEPS: [u32; 6] = [16, 8, 4, 2, 1, 1];

/// Background (empty air) colour at the top and bottom of the grid, linear.
pub const BG_TOP: [f32; 3] = [0.008, 0.011, 0.022];
/// See [`BG_TOP`].
pub const BG_BOTTOM: [f32; 3] = [0.020, 0.024, 0.036];
/// Vapor opacity per unit vapor: `alpha = 1 - exp(-VAPOR_K * vapor)`.
pub const VAPOR_K: f32 = 2.0;
/// Steam and vapor colour (linear).
pub const STEAM_RGB: [f32; 3] = [0.80, 0.82, 0.85];
/// Minimum opacity of a Steam cell (scaled by noise 0.8..1.2).
pub const STEAM_ALPHA: f32 = 0.55;
/// Smoke colour (linear).
pub const SMOKE_RGB: [f32; 3] = [0.09, 0.09, 0.10];
/// Minimum opacity of a Smoke cell (scaled by noise 0.8..1.2).
pub const SMOKE_ALPHA: f32 = 0.75;
/// Fire emission is scaled by `FIRE_FLICKER + (1 - FIRE_FLICKER) * noise`.
pub const FIRE_FLICKER: f32 = 0.6;

/// Liquid cells counted above a cell for depth darkening.
pub const DEPTH_MAX: i32 = 12;
/// Water absorption per cell of depth (red absorbed most, so deep water is
/// blue; Beer-Lambert, ratios after Pope & Fry 1997, compressed).
pub const ABSORB_WATER: [f32; 3] = [0.30, 0.10, 0.03];
/// Absorption per cell for other liquids (oil, acid, lava).
pub const ABSORB_OTHER: [f32; 3] = [0.08, 0.08, 0.08];
/// Cells searched up/down a neighbouring column for its liquid surface.
pub const SURF_SEARCH: i32 = 4;
/// Direction toward the sky light, +y down (from the upper left).
pub const SUN_DIR: [f32; 2] = [-0.371_390_7, -0.928_476_7];
/// Surface highlight colour, strength and exponent:
/// `SURF_RGB * SURF_SPEC * max(n . SUN_DIR, 0)^SURF_POW`.
pub const SURF_RGB: [f32; 3] = [0.55, 0.65, 0.75];
/// See [`SURF_RGB`].
pub const SURF_SPEC: f32 = 0.8;
/// See [`SURF_RGB`].
pub const SURF_POW: f32 = 16.0;

/// Width in pixels over which a cell boundary blends (sharpened bilinear).
pub const EDGE_PX: f32 = 2.0;
/// Most pixels per cell in the HDR scene.
pub const MAX_SCALE: u32 = 4;
/// Longest side of the HDR scene in pixels (bounds memory at 1024 x 1024).
pub const MAX_SCENE_SIDE: u32 = 2048;
/// Sprite radius of a particle in cells.
pub const PART_RADIUS: f32 = 0.6;
/// Sprite radius of a particle hotter than [`EMIT_MIN_K`] in cells.
pub const PART_GLOW_RADIUS: f32 = 1.5;
/// Particles drawn per frame: every occupied slot, one instanced draw.
pub const PARTICLE_DRAW_CAP: u32 = Particles::MAX;

/// Luminance above which the scene feeds the bloom (HDR units).
pub const BLOOM_THRESHOLD: f32 = 1.0;
/// Gaussian sigma of the bloom blur, in quarter-resolution texels.
pub const BLOOM_SIGMA: f32 = 2.0;
/// Bloom blur radius in taps each side (9-tap kernel).
pub const BLOOM_RADIUS: i32 = 4;
/// Bloom added to the scene before tone mapping.
pub const BLOOM_STRENGTH: f32 = 0.6;
/// Exposure applied before the ACES curve.
pub const EXPOSURE: f32 = 1.0;

/// Linear albedo (rgb) and noise amplitude (`1 +- amp`) per material id,
/// indexed by id, with [`WALL`] last. Hand-picked linear values.
pub const ALBEDO: [[f32; 4]; MATERIAL_COUNT + 1] = [
    [0.0, 0.0, 0.0, 0.0],     // Empty (background, see BG_TOP)
    [0.69, 0.48, 0.16, 0.25], // Sand
    [0.02, 0.10, 0.35, 0.04], // Water
    [0.12, 0.07, 0.02, 0.08], // Oil
    [0.22, 0.22, 0.24, 0.20], // Stone
    [0.12, 0.11, 0.11, 0.20], // Rock
    [0.25, 0.12, 0.05, 0.25], // Wood
    [0.30, 0.30, 0.30, 0.20], // Ash
    [0.45, 0.47, 0.50, 0.05], // Metal
    [0.55, 0.70, 0.75, 0.05], // Glass
    [0.35, 0.05, 0.01, 0.30], // Lava (crust; emission adds the glow)
    [0.15, 0.55, 0.05, 0.08], // Acid
    [0.08, 0.08, 0.08, 0.30], // Gunpowder
    [0.60, 0.80, 0.95, 0.05], // Ice
    [0.90, 0.30, 0.05, 0.30], // Fire
    [0.80, 0.82, 0.85, 0.10], // Steam
    [0.09, 0.09, 0.10, 0.10], // Smoke
    [0.40, 0.10, 0.02, 0.30], // Ember
    [0.08, 0.08, 0.09, 0.10], // WALL
];

/// Pixels per cell of the HDR scene for a `w` x `h` grid:
/// `MAX_SCENE_SIDE / max(w, h)` clamped to `1..=MAX_SCALE`.
pub fn scene_scale(w: u32, h: u32) -> u32 {
    (MAX_SCENE_SIDE / w.max(h)).clamp(1, MAX_SCALE)
}

const HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

fn f(x: f32) -> String {
    format!("{x:?}")
}

fn v3(c: [f32; 3]) -> String {
    format!("vec3<f32>({}, {}, {})", f(c[0]), f(c[1]), f(c[2]))
}

/// WGSL header with every look constant and table.
fn header() -> String {
    use Material::*;
    let solids: Vec<String> = (0..=WALL)
        .filter(|&m| is_solid(m))
        .map(|m| format!("m == {m}u"))
        .collect();
    let albedo: Vec<String> = ALBEDO
        .iter()
        .map(|a| {
            format!(
                "vec4<f32>({}, {}, {}, {})",
                f(a[0]),
                f(a[1]),
                f(a[2]),
                f(a[3])
            )
        })
        .collect();
    let bb_t: Vec<String> = BLACKBODY.iter().map(|b| f(b.0)).collect();
    let bb_c: Vec<String> = BLACKBODY.iter().map(|b| v3(b.1)).collect();
    let mut s = String::new();
    for (name, id) in [
        ("EMPTY", Empty),
        ("WATER", Water),
        ("OIL", Oil),
        ("LAVA", Lava),
        ("ACID", Acid),
        ("FIRE", Fire),
        ("STEAM", Steam),
        ("SMOKE", Smoke),
    ] {
        s += &format!("const {name}: u32 = {}u;\n", id as u32);
    }
    s += &format!("const WALL: u32 = {WALL}u;\nconst NOISE_SEED: u32 = {NOISE_SEED}u;\n");
    for (name, v) in [
        ("AMBIENT", AMBIENT),
        ("LIGHT_GAIN", LIGHT_GAIN),
        ("AIR_GLOW", AIR_GLOW),
        ("AMBIENT_K", AMBIENT_K),
        ("EMIT_MIN_K", EMIT_MIN_K),
        ("EMIT_FULL_K", EMIT_FULL_K),
        ("EMIT_REF_K", EMIT_REF_K),
        ("EMIT_GAIN", EMIT_GAIN),
        ("EMIT_MAX", EMIT_MAX),
        ("LIGHT_RADIUS", LIGHT_RADIUS),
        ("LIGHT_D0", LIGHT_D0),
        ("VAPOR_K", VAPOR_K),
        ("STEAM_ALPHA", STEAM_ALPHA),
        ("SMOKE_ALPHA", SMOKE_ALPHA),
        ("FIRE_FLICKER", FIRE_FLICKER),
        ("SURF_SPEC", SURF_SPEC),
        ("SURF_POW", SURF_POW),
        ("EDGE_PX", EDGE_PX),
        ("PART_RADIUS", PART_RADIUS),
        ("PART_GLOW_RADIUS", PART_GLOW_RADIUS),
        ("BLOOM_THRESHOLD", BLOOM_THRESHOLD),
        ("BLOOM_SIGMA", BLOOM_SIGMA),
        ("BLOOM_STRENGTH", BLOOM_STRENGTH),
        ("EXPOSURE", EXPOSURE),
    ] {
        s += &format!("const {name}: f32 = {};\n", f(v));
    }
    for (name, v) in [
        ("BG_TOP", BG_TOP),
        ("BG_BOTTOM", BG_BOTTOM),
        ("STEAM_RGB", STEAM_RGB),
        ("SMOKE_RGB", SMOKE_RGB),
        ("ABSORB_WATER", ABSORB_WATER),
        ("ABSORB_OTHER", ABSORB_OTHER),
        ("SURF_RGB", SURF_RGB),
    ] {
        s += &format!("const {name}: vec3<f32> = {};\n", v3(v));
    }
    s += &format!(
        "const SUN_DIR: vec2<f32> = vec2<f32>({}, {});\n\
         const DEPTH_MAX: i32 = {DEPTH_MAX};\nconst SURF_SEARCH: i32 = {SURF_SEARCH};\n\
         const BLOOM_RADIUS: i32 = {BLOOM_RADIUS};\nconst BB_N: i32 = {};\n\
         const BB_T = array<f32, {}>({});\nconst BB_C = array<vec3<f32>, {}>({});\n\
         const ALBEDO = array<vec4<f32>, {}>({});\n\
         fn is_solid(m: u32) -> bool {{ return {}; }}\n\
         fn is_liquid(m: u32) -> bool {{ return m == WATER || m == OIL || m == ACID || m == LAVA; }}\n",
        f(SUN_DIR[0]),
        f(SUN_DIR[1]),
        BLACKBODY.len(),
        BLACKBODY.len(),
        bb_t.join(", "),
        BLACKBODY.len(),
        bb_c.join(", "),
        ALBEDO.len(),
        albedo.join(", "),
        solids.join(" || "),
    );
    s
}

fn module(gpu: &Gpu, label: &str, body: &str, extra: &str) -> wgpu::ShaderModule {
    let src = [
        header().as_str(),
        extra,
        include_str!("shaders/common.wgsl"),
        include_str!("shaders/hash.wgsl"),
        include_str!("shaders/look.wgsl"),
        include_str!("shaders/fullscreen.wgsl"),
        body,
    ]
    .join("\n");
    gpu.device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        })
}

/// Layout entry for a uniform or storage buffer.
pub(crate) fn buffer_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    ty: wgpu::BufferBindingType,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// Layout entry for a filterable float 2D texture.
pub(crate) fn texture_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// Layout entry for a write-only `rgba16float` storage texture (compute).
pub(crate) fn storage_tex_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: HDR,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn pipeline_layout(gpu: &Gpu, layout: &wgpu::BindGroupLayout) -> wgpu::PipelineLayout {
    gpu.device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(layout)],
            immediate_size: 0,
        })
}

/// A compute pipeline for `entry` of `module` with one bind group `layout`.
pub(crate) fn compute_pipeline(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    module: &wgpu::ShaderModule,
    entry: &str,
) -> wgpu::ComputePipeline {
    gpu.device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(entry),
            layout: Some(&pipeline_layout(gpu, layout)),
            module,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            cache: None,
        })
}

struct Raster<'a> {
    module: &'a wgpu::ShaderModule,
    vs: &'a str,
    fs: &'a str,
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
    buffers: &'a [Option<wgpu::VertexBufferLayout<'a>>],
    topology: wgpu::PrimitiveTopology,
}

fn render_pipeline(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    r: Raster<'_>,
) -> wgpu::RenderPipeline {
    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(r.fs),
            layout: Some(&pipeline_layout(gpu, layout)),
            vertex: wgpu::VertexState {
                module: r.module,
                entry_point: Some(r.vs),
                compilation_options: Default::default(),
                buffers: r.buffers,
            },
            primitive: wgpu::PrimitiveState {
                topology: r.topology,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: r.module,
                entry_point: Some(r.fs),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: r.format,
                    blend: r.blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
}

fn fullscreen(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    module: &wgpu::ShaderModule,
    fs: &str,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    render_pipeline(
        gpu,
        layout,
        Raster {
            module,
            vs: "fullscreen",
            fs,
            format,
            blend: None,
            buffers: &[],
            topology: wgpu::PrimitiveTopology::TriangleList,
        },
    )
}

pub(crate) fn texture(
    gpu: &Gpu,
    label: &str,
    w: u32,
    h: u32,
    usage: wgpu::TextureUsages,
) -> wgpu::TextureView {
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDR,
            usage: usage | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

fn group(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    res: &[wgpu::BindingResource<'_>],
) -> wgpu::BindGroup {
    let entries: Vec<wgpu::BindGroupEntry<'_>> = res
        .iter()
        .enumerate()
        .map(|(i, r)| wgpu::BindGroupEntry {
            binding: i as u32,
            resource: r.clone(),
        })
        .collect();
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &entries,
    })
}

/// Grid-sized textures and the bind groups that use them.
struct Sized {
    grid_id: u64,
    w: u32,
    h: u32,
    scene: wgpu::TextureView,
    bloom: [wgpu::TextureView; 2],
    light: LightTargets,
    shade: [wgpu::BindGroup; 2],
    scene_group: wgpu::BindGroup,
    down: wgpu::BindGroup,
    blur_h: wgpu::BindGroup,
    blur_v: wgpu::BindGroup,
    composite: wgpu::BindGroup,
}

/// Persistent render pipelines plus grid-sized textures (rebuilt only when
/// the grid changes).
pub struct Renderer {
    light: LightPass,
    shade: wgpu::ComputePipeline,
    shade_layout: wgpu::BindGroupLayout,
    fill: wgpu::RenderPipeline,
    sprite: wgpu::RenderPipeline,
    scene_layout: wgpu::BindGroupLayout,
    down: wgpu::RenderPipeline,
    blur_h: wgpu::RenderPipeline,
    blur_v: wgpu::RenderPipeline,
    bloom_layout: wgpu::BindGroupLayout,
    composite: wgpu::RenderPipeline,
    composite_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    view_ubo: wgpu::Buffer,
    target_ubo: wgpu::Buffer,
    sized: Option<Sized>,
}

impl Renderer {
    /// Compiles every pipeline once for targets of `format`. A non-sRGB
    /// format gets the sRGB transfer applied in the composite shader.
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Renderer {
        use wgpu::BufferBindingType::{Storage, Uniform};
        let d = &gpu.device;
        let cs = wgpu::ShaderStages::COMPUTE;
        let vf = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let fs = wgpu::ShaderStages::FRAGMENT;

        let light_mod = module(gpu, "light", include_str!("shaders/light.wgsl"), "");
        let shade_mod = module(gpu, "shade", include_str!("shaders/shade.wgsl"), "");
        let scene_mod = module(gpu, "scene", include_str!("shaders/scene.wgsl"), "");
        let bloom_mod = module(gpu, "bloom", include_str!("shaders/bloom.wgsl"), "");
        let srgb = format!("const SRGB_ENCODE: bool = {};", !format.is_srgb());
        let comp_mod = module(
            gpu,
            "composite",
            include_str!("shaders/composite.wgsl"),
            &srgb,
        );

        let shade_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shade"),
            entries: &[
                buffer_entry(0, cs, Uniform),
                buffer_entry(1, cs, Storage { read_only: true }),
                texture_entry(2, cs),
                storage_tex_entry(3),
            ],
        });
        let scene_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene"),
            entries: &[
                buffer_entry(0, vf, Uniform),
                texture_entry(1, fs),
                sampler_entry(2),
                texture_entry(3, wgpu::ShaderStages::VERTEX),
            ],
        });
        let bloom_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloom"),
            entries: &[texture_entry(0, fs), sampler_entry(1)],
        });
        let composite_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("composite"),
            entries: &[
                texture_entry(0, fs),
                texture_entry(1, fs),
                sampler_entry(2),
                buffer_entry(3, fs, Uniform),
            ],
        });

        let sprite_attrs = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: 16,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 20,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 24,
                shader_location: 3,
            },
        ];
        let sprite = render_pipeline(
            gpu,
            &scene_layout,
            Raster {
                module: &scene_mod,
                vs: "sprite_vs",
                fs: "sprite_fs",
                format: HDR,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<crate::ParticleRaw>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &sprite_attrs,
                })],
                topology: wgpu::PrimitiveTopology::TriangleStrip,
            },
        );

        let sampler = d.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear_clamp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let ubo = |label, size| {
            d.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Renderer {
            light: LightPass::new(gpu, &light_mod),
            shade: compute_pipeline(gpu, &shade_layout, &shade_mod, "shade"),
            shade_layout,
            fill: fullscreen(gpu, &scene_layout, &scene_mod, "fill_fs", HDR),
            sprite,
            scene_layout,
            down: fullscreen(gpu, &bloom_layout, &bloom_mod, "down", HDR),
            blur_h: fullscreen(gpu, &bloom_layout, &bloom_mod, "blur_h", HDR),
            blur_v: fullscreen(gpu, &bloom_layout, &bloom_mod, "blur_v", HDR),
            bloom_layout,
            composite: fullscreen(gpu, &composite_layout, &comp_mod, "composite", format),
            composite_layout,
            sampler,
            view_ubo: ubo("render_view", 16),
            target_ubo: ubo("render_target", 16),
            sized: None,
        }
    }

    /// (Re)creates the grid-sized textures and bind groups for `grid`.
    fn ensure_sized(&mut self, gpu: &Gpu, grid: &GridState) {
        let (w, h) = (grid.width(), grid.height());
        if self
            .sized
            .as_ref()
            .is_some_and(|s| s.grid_id == grid.id() && s.w == w && s.h == h)
        {
            return;
        }
        let scale = scene_scale(w, h);
        let (scene_w, scene_h) = (w * scale, h * scale);
        let (bloom_w, bloom_h) = (scene_w.div_ceil(4), scene_h.div_ceil(4));
        gpu.queue.write_buffer(
            &self.view_ubo,
            0,
            bytemuck::cast_slice(&[w, h, scale, NOISE_SEED]),
        );
        use wgpu::TextureUsages as U;
        let storage = U::STORAGE_BINDING;
        let light = self.light.targets(gpu, &self.view_ubo, grid);
        let cellcol = texture(gpu, "cell_colour", w, h, storage);
        let scene = texture(gpu, "scene_hdr", scene_w, scene_h, U::RENDER_ATTACHMENT);
        let bloom = [
            texture(gpu, "bloom_0", bloom_w, bloom_h, U::RENDER_ATTACHMENT),
            texture(gpu, "bloom_1", bloom_w, bloom_h, U::RENDER_ATTACHMENT),
        ];
        use wgpu::BindingResource as R;
        let tv = R::TextureView;
        let ubo = self.view_ubo.as_entire_binding();
        let samp = R::Sampler(&self.sampler);
        let shade = [0, 1].map(|cur| {
            group(
                gpu,
                &self.shade_layout,
                &[
                    ubo.clone(),
                    grid.buffer(cur).as_entire_binding(),
                    tv(&light.light),
                    tv(&cellcol),
                ],
            )
        });
        let scene_group = group(
            gpu,
            &self.scene_layout,
            &[ubo.clone(), tv(&cellcol), samp.clone(), tv(&light.light)],
        );
        let down = group(gpu, &self.bloom_layout, &[tv(&scene), samp.clone()]);
        let blur_h = group(gpu, &self.bloom_layout, &[tv(&bloom[0]), samp.clone()]);
        let blur_v = group(gpu, &self.bloom_layout, &[tv(&bloom[1]), samp.clone()]);
        let composite = group(
            gpu,
            &self.composite_layout,
            &[
                tv(&scene),
                tv(&bloom[0]),
                samp,
                self.target_ubo.as_entire_binding(),
            ],
        );
        self.sized = Some(Sized {
            grid_id: grid.id(),
            w,
            h,
            scene,
            bloom,
            light,
            shade,
            scene_group,
            down,
            blur_h,
            blur_v,
            composite,
        });
    }

    /// Renders `grid`'s current buffer and `parts` into `target` (any size;
    /// the image is stretched to fill it). One submit. Does nothing for a
    /// zero-size target.
    pub fn frame(
        &mut self,
        gpu: &Gpu,
        grid: &GridState,
        parts: &Particles,
        target: &wgpu::TextureView,
    ) {
        let size = target.texture().size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.ensure_sized(gpu, grid);
        let s = self.sized.as_ref().expect("sized just ensured");
        let cur = grid.current_index();
        gpu.queue.write_buffer(
            &self.target_ubo,
            0,
            bytemuck::cast_slice(&[size.width as f32, size.height as f32, 0.0, 0.0]),
        );
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            self.light.record(&mut pass, &s.light, cur, s.w, s.h);
            pass.set_pipeline(&self.shade);
            pass.set_bind_group(0, &s.shade[cur], &[]);
            pass.dispatch_workgroups(s.w.div_ceil(8), s.h.div_ceil(8), 1);
        }
        {
            let mut rp = begin(&mut enc, &s.scene);
            rp.set_bind_group(0, &s.scene_group, &[]);
            rp.set_pipeline(&self.fill);
            rp.draw(0..3, 0..1);
            let n = parts.count().min(PARTICLE_DRAW_CAP);
            if n > 0 {
                rp.set_pipeline(&self.sprite);
                rp.set_vertex_buffer(0, parts.buffer().slice(..));
                rp.draw(0..4, 0..n);
            }
        }
        for (pipe, bind, dst) in [
            (&self.down, &s.down, &s.bloom[0]),
            (&self.blur_h, &s.blur_h, &s.bloom[1]),
            (&self.blur_v, &s.blur_v, &s.bloom[0]),
        ] {
            let mut rp = begin(&mut enc, dst);
            rp.set_pipeline(pipe);
            rp.set_bind_group(0, bind, &[]);
            rp.draw(0..3, 0..1);
        }
        {
            let mut rp = begin(&mut enc, target);
            rp.set_pipeline(&self.composite);
            rp.set_bind_group(0, &s.composite, &[]);
            rp.draw(0..3, 0..1);
        }
        gpu.queue.submit(Some(enc.finish()));
    }
}

fn begin<'e>(
    enc: &'e mut wgpu::CommandEncoder,
    view: &'e wgpu::TextureView,
) -> wgpu::RenderPass<'e> {
    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}
