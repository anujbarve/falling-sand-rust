//! Drawing the world entirely on the GPU (OpenGL, via macroquad).
//!
//! The simulation's cells arrive as a `&[u32]` readback. They are uploaded
//! as they are: one cell = one RGBA8 texel (bytes = material/aux/temperature
//! bits). No per-pixel work happens on the CPU. Fragment shaders decode the
//! bits and do all the looks:
//!
//! 1. `emit`: what glows (lava, flames, incandescent solids, hot air), into
//!    a small render target (1/2, 1/4 or 1/8 of the grid).
//! 2. `blur`: a 9-tap Gaussian, horizontally then vertically (separable:
//!    2 x 9 samples instead of 81), on that small target.
//! 3. `scene`: the final picture at window resolution: material colour,
//!    grain, surface lighting and contact shadows, liquid depth, black-body
//!    glow, plus the blurred bloom, tone mapping and vignette.
//!
//! Render targets are only re-created when the grid size or the bloom size
//! changes, never per frame.

use macroquad::prelude::*;

use crate::settings::Graphics;

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
varying highp vec2 uv;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    uv = texcoord;
}
"#;

/// Shared by all fragment shaders: decoding a cell (see "Materials" in
/// physics/mod.rs) and the material colours. GLSL 100 has no integer bit
/// operations, so the bytes are taken apart with float maths; every value
/// involved is below 2^24, which a highp float holds exactly.
const COMMON: &str = r#"#version 100
precision highp float;
varying highp vec2 uv;
uniform sampler2D Texture;
uniform vec2 GridSize;

vec4 cell_at(vec2 c) {
    c = clamp(c, vec2(0.0), GridSize - 1.0);
    return floor(texture2D(Texture, (c + 0.5) / GridSize) * 255.0 + 0.5);
}
float mat_of(vec4 c) { return mod(c.r, 16.0); }
float aux_of(vec4 c) { return floor(c.r / 16.0) + mod(c.g, 16.0) * 16.0; }
// The raw temperature field in °C (includes stored latent heat).
float temp_raw(vec4 c) {
    float t = floor(c.g / 16.0) + c.b * 16.0 + c.a * 4096.0;
    if (t >= 524288.0) { t -= 1048576.0; }
    return t / 256.0;
}
// The real temperature in °C.
float temp_of(vec4 c) {
    float t = temp_raw(c);
    // Water's field also stores latent heat (boiling above 100, freezing below
    // 0): its real temperature is pinned to 0..100°C (sens() in materials.rs).
    if (abs(mod(c.r, 16.0) - 3.0) < 0.5) { t = clamp(t, 0.0, 100.0); }
    if (abs(mod(c.r, 16.0) - 6.0) < 0.5) { t = max(t, 100.0); } // wet steam parcel
    if (abs(mod(c.r, 16.0) - 9.0) < 0.5) { t = min(t, 0.0); }   // melting ice
    return t;
}
bool is_mat(float m, float k) { return abs(m - k) < 0.5; }
// AIR 0 STONE 1 SAND 2 WATER 3 OIL 4 LAVA 5 STEAM 6 WOOD 7 FIRE 8 ICE 9 SMOKE 10
bool is_gas(float m) { return is_mat(m, 0.0) || is_mat(m, 6.0) || is_mat(m, 8.0) || is_mat(m, 10.0); }
bool is_liquid(float m) { return is_mat(m, 3.0) || is_mat(m, 4.0) || is_mat(m, 5.0); }

float hash(vec2 p) { return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453); }

// Black-body-ish colour for a temperature in °C (dull red .. white).
vec3 heat_tint(float t) {
    float k = clamp((t - 500.0) / 1100.0, 0.0, 1.0);
    return mix(vec3(0.55, 0.05, 0.0), mix(vec3(1.0, 0.45, 0.05), vec3(1.0, 0.95, 0.75), k), k);
}

vec3 lava_color(float t) {
    float k = clamp((t - 950.0) / 250.0, 0.0, 1.0);
    return mix(vec3(0.45, 0.06, 0.02), vec3(1.0, 0.55, 0.12), k);
}
vec3 fire_color(float life) {
    return mix(vec3(0.85, 0.15, 0.03), vec3(1.0, 0.8, 0.3), clamp(life / 120.0, 0.0, 1.0));
}

// How much a cell glows by itself (fed into the bloom).
vec3 emission(vec4 c, float incandescence) {
    float m = mat_of(c);
    float t = temp_of(c);
    if (is_mat(m, 5.0)) { return lava_color(t) * 1.1; }
    if (is_mat(m, 8.0)) { return fire_color(aux_of(c)) * 1.3; }
    if (!is_gas(m) && t > 500.0) { return heat_tint(t) * clamp((t - 500.0) / 700.0, 0.0, 1.0) * incandescence; }
    if (is_mat(m, 0.0) && t > 150.0) { return vec3(0.9, 0.3, 0.1) * clamp((t - 150.0) / 1500.0, 0.0, 0.25); }
    return vec3(0.0);
}
"#;

const EMIT: &str = r#"
uniform vec4 Params; // x: incandescence on, y: cells per bloom texel
void main() {
    // Average 4 cells per bloom texel so small sparks do not flicker.
    vec2 c = uv * GridSize;
    float q = Params.y * 0.25;
    vec3 e = emission(cell_at(c + vec2(-q, -q)), Params.x) + emission(cell_at(c + vec2(q, -q)), Params.x)
           + emission(cell_at(c + vec2(-q, q)), Params.x) + emission(cell_at(c + vec2(q, q)), Params.x);
    gl_FragColor = vec4(e * 0.25, 1.0);
}
"#;

const BLUR: &str = r#"#version 100
precision highp float;
varying highp vec2 uv;
uniform sampler2D Texture;
uniform vec2 Step; // one texel along the blur direction
void main() {
    // Render targets are stored upside down: flip on read.
    vec2 p = vec2(uv.x, 1.0 - uv.y);
    vec3 s = texture2D(Texture, p).rgb * 0.2270270;
    s += (texture2D(Texture, p + Step).rgb + texture2D(Texture, p - Step).rgb) * 0.1945946;
    s += (texture2D(Texture, p + 2.0 * Step).rgb + texture2D(Texture, p - 2.0 * Step).rgb) * 0.1216216;
    s += (texture2D(Texture, p + 3.0 * Step).rgb + texture2D(Texture, p - 3.0 * Step).rgb) * 0.0540541;
    s += (texture2D(Texture, p + 4.0 * Step).rgb + texture2D(Texture, p - 4.0 * Step).rgb) * 0.0162162;
    gl_FragColor = vec4(s, 1.0);
}
"#;

const SCENE: &str = r#"
uniform sampler2D Glow;
uniform vec4 Flags;  // shading, depth, grain, incandescence
uniform vec4 Flags2; // bloom strength, exposure, vignette, heat view
uniform float Time;

vec3 heat_view(float t) {
    if (t < 20.0) { return mix(vec3(0.04, 0.04, 0.08), vec3(0.25, 0.55, 1.0), clamp((20.0 - t) / 60.0, 0.0, 1.0)); }
    if (t < 200.0) { return mix(vec3(0.04, 0.04, 0.08), vec3(0.8, 0.12, 0.08), (t - 20.0) / 180.0); }
    if (t < 900.0) { return mix(vec3(0.8, 0.12, 0.08), vec3(1.0, 0.86, 0.16), (t - 200.0) / 700.0); }
    return mix(vec3(1.0, 0.86, 0.16), vec3(1.0), clamp((t - 900.0) / 700.0, 0.0, 1.0));
}

void main() {
    vec2 cp = floor(uv * GridSize);
    vec4 c = cell_at(cp);
    float m = mat_of(c);
    float t = temp_of(c);
    float a = aux_of(c);

    if (Flags2.w > 0.5) {
        gl_FragColor = vec4(heat_view(t), 1.0);
        return;
    }

    // Background: a dark night-sky gradient behind gases.
    vec3 sky = mix(vec3(0.06, 0.07, 0.11), vec3(0.015, 0.015, 0.025), uv.y);
    float n = Flags.z > 0.5 ? hash(cp) - 0.5 : 0.0; // stable per grain
    vec3 col;
    if (is_mat(m, 0.0)) {
        col = sky + vec3(0.35, 0.08, 0.02) * clamp((t - 40.0) / 800.0, 0.0, 1.0); // hot air shimmer
    } else if (is_mat(m, 1.0)) { col = vec3(0.43, 0.43, 0.46) * (1.0 + n * 0.18); }
    else if (is_mat(m, 2.0)) { col = vec3(0.86, 0.74, 0.45) * (1.0 + n * 0.22); }
    else if (is_mat(m, 3.0)) { col = vec3(0.13, 0.36, 0.78); }
    else if (is_mat(m, 4.0)) { col = vec3(0.40, 0.29, 0.10); }
    else if (is_mat(m, 5.0)) { col = lava_color(t) * (1.0 + n * 0.25); }
    else if (is_mat(m, 6.0)) {
        // Steam: bright billowing white, thinner where the parcel is wet
        // (partly condensed: field below 100°C).
        float dry = clamp((temp_raw(c) - 100.0 + 541.0) / 541.0, 0.0, 1.0);
        col = mix(sky, vec3(0.88, 0.9, 0.94), 0.55 + 0.3 * dry + n * 0.15);
    }
    else if (is_mat(m, 7.0)) { col = vec3(0.42, 0.26, 0.13) * (1.0 + n * 0.2 + 0.08 * sin(cp.y * 0.9)); }
    else if (is_mat(m, 8.0)) { col = fire_color(a) * (1.0 + 0.15 * sin(Time * 30.0 + cp.x * 1.7 + cp.y)); }
    else if (is_mat(m, 9.0)) { col = vec3(0.72, 0.86, 0.98) * (1.0 + n * 0.08); }
    else { col = mix(sky, vec3(0.22, 0.22, 0.24), 0.55 + n * 0.2); } // smoke

    bool solidish = !is_gas(m);
    if (Flags.x > 0.5) {
        float up = is_gas(mat_of(cell_at(cp + vec2(0.0, -1.0)))) ? 1.0 : 0.0;
        float dn = is_gas(mat_of(cell_at(cp + vec2(0.0, 1.0)))) ? 1.0 : 0.0;
        float lf = is_gas(mat_of(cell_at(cp + vec2(-1.0, 0.0)))) ? 1.0 : 0.0;
        float rt = is_gas(mat_of(cell_at(cp + vec2(1.0, 0.0)))) ? 1.0 : 0.0;
        if (solidish) {
            // Light from the upper left: lit top/left faces, shaded bottom/right.
            col *= 1.0 + 0.22 * up + 0.08 * lf - 0.12 * dn - 0.06 * rt;
        } else if (is_mat(m, 0.0)) {
            // Contact shadow: air right under or beside matter is a bit darker.
            col *= 1.0 - 0.25 * (1.0 - up) - 0.08 * ((1.0 - lf) + (1.0 - rt));
        }
    }
    if (is_liquid(m) && !is_mat(m, 5.0)) {
        if (Flags.y > 0.5) {
            // Deeper liquid is darker: look a few distances straight up.
            // The distances are jittered per cell, so instead of hard bands
            // at fixed depths the darkening fades in smoothly.
            float j = 0.6 + 0.8 * hash(cp + 17.0);
            float d = 0.0;
            d += is_liquid(mat_of(cell_at(cp + vec2(0.0, -floor(3.0 * j))))) ? 1.0 : 0.0;
            d += is_liquid(mat_of(cell_at(cp + vec2(0.0, -floor(8.0 * j))))) ? 1.0 : 0.0;
            d += is_liquid(mat_of(cell_at(cp + vec2(0.0, -floor(16.0 * j))))) ? 1.0 : 0.0;
            d += is_liquid(mat_of(cell_at(cp + vec2(0.0, -floor(32.0 * j))))) ? 1.0 : 0.0;
            col *= 1.0 - 0.11 * d;
        }
        if (Flags.z > 0.5) {
            col *= 1.0 + 0.05 * sin(Time * 2.5 + cp.x * 0.35 + cp.y * 0.6); // shimmer
        }
        // Bright line on the surface.
        if (is_gas(mat_of(cell_at(cp + vec2(0.0, -1.0))))) { col += vec3(0.12, 0.16, 0.2); }
    }
    if (is_mat(m, 3.0)) {
        // A steam bubble is smaller than a 5 mm cell. Show the unresolved
        // boiling layer only where water borders an incandescent surface.
        vec4 n0 = cell_at(cp + vec2(1.0, 0.0));
        vec4 n1 = cell_at(cp + vec2(-1.0, 0.0));
        vec4 n2 = cell_at(cp + vec2(0.0, 1.0));
        vec4 n3 = cell_at(cp + vec2(0.0, -1.0));
        float hot = 0.0;
        if (!is_gas(mat_of(n0))) { hot = max(hot, temp_of(n0)); }
        if (!is_gas(mat_of(n1))) { hot = max(hot, temp_of(n1)); }
        if (!is_gas(mat_of(n2))) { hot = max(hot, temp_of(n2)); }
        if (!is_gas(mat_of(n3))) { hot = max(hot, temp_of(n3)); }
        float boil = clamp((hot - 350.0) / 450.0, 0.0, 1.0);
        float fleck = step(0.54, hash(floor(cp * 0.8 + vec2(0.0, Time * 18.0))));
        col = mix(col, vec3(0.55, 0.85, 1.0), boil * (0.35 + 0.5 * fleck));
    }
    if (Flags.w > 0.5 && solidish && !is_mat(m, 5.0) && t > 450.0) {
        col = mix(col, heat_tint(t), clamp((t - 450.0) / 650.0, 0.0, 1.0));
    }

    // Bloom (stored upside down, like every render target).
    col += texture2D(Glow, vec2(uv.x, 1.0 - uv.y)).rgb * Flags2.x;

    // Tone mapping: compresses only what goes above ~0.8, so bright glow
    // saturates smoothly instead of clipping, and darks stay put.
    col *= Flags2.y;
    vec3 over = max(col - 0.8, 0.0);
    col = min(col, 0.8) + 0.2 * (1.0 - exp(-over * 5.0));

    if (Flags2.z > 0.5) {
        float v = length(uv - 0.5) * 1.25;
        col *= mix(1.0, 0.72, smoothstep(0.45, 0.95, v));
    }
    gl_FragColor = vec4(col, 1.0);
}
"#;

pub struct Renderer {
    grid: (usize, usize),
    bloom_divisor: u32,
    cells: Texture2D,
    emit: Material,
    blur: Material,
    scene: Material,
    glow: [RenderTarget; 2],
}

fn material(fragment: &str, uniforms: &[(&str, UniformType)], textures: &[&str]) -> Material {
    load_material(
        ShaderSource::Glsl { vertex: VERTEX, fragment },
        MaterialParams {
            uniforms: uniforms.iter().map(|&(n, t)| UniformDesc::new(n, t)).collect(),
            textures: textures.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        },
    )
    .unwrap_or_else(|e| panic!("shader failed to compile: {e:?}"))
}

fn glow_targets(grid: (usize, usize), divisor: u32) -> [RenderTarget; 2] {
    let (w, h) = ((grid.0 as u32).div_ceil(divisor).max(1), (grid.1 as u32).div_ceil(divisor).max(1));
    let make = || {
        let rt = render_target(w, h);
        rt.texture.set_filter(FilterMode::Linear); // smooth when stretched up
        rt
    };
    [make(), make()]
}

/// Draw `source` over the whole of `target` with `mat`.
fn pass(mat: &Material, source: &Texture2D, target: &RenderTarget) {
    let (w, h) = (target.texture.width(), target.texture.height());
    let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, w, h));
    cam.render_target = Some(target.clone());
    set_camera(&cam);
    clear_background(BLACK);
    gl_use_material(mat);
    draw_texture_ex(source, 0.0, 0.0, WHITE, DrawTextureParams { dest_size: Some(vec2(w, h)), ..Default::default() });
    gl_use_default_material();
    set_default_camera();
}

impl Renderer {
    pub fn new(grid: (usize, usize), g: &Graphics) -> Renderer {
        let emit_src = format!("{COMMON}{EMIT}");
        let scene_src = format!("{COMMON}{SCENE}");
        let emit = material(&emit_src, &[("GridSize", UniformType::Float2), ("Params", UniformType::Float4)], &[]);
        let blur = material(BLUR, &[("Step", UniformType::Float2)], &[]);
        let scene = material(
            &scene_src,
            &[
                ("GridSize", UniformType::Float2),
                ("Flags", UniformType::Float4),
                ("Flags2", UniformType::Float4),
                ("Time", UniformType::Float1),
            ],
            &["Glow"],
        );
        let cells = Texture2D::from_rgba8(grid.0 as u16, grid.1 as u16, &vec![0; grid.0 * grid.1 * 4]);
        cells.set_filter(FilterMode::Nearest);
        Renderer { grid, bloom_divisor: g.bloom_divisor, cells, emit, blur, scene, glow: glow_targets(grid, g.bloom_divisor) }
    }

    /// Upload a new frame of cells. `bytemuck` reinterprets the u32s as
    /// bytes without copying them first; the GPU upload is the only copy.
    pub fn upload(&mut self, cells: &[u32]) {
        debug_assert_eq!(cells.len(), self.grid.0 * self.grid.1);
        self.cells.update_from_bytes(self.grid.0 as u32, self.grid.1 as u32, bytemuck::cast_slice(cells));
    }

    /// Draw the world into `dest` (screen pixels).
    pub fn draw(&mut self, dest: Rect, g: &Graphics, heat_view: bool, time: f32) {
        if g.bloom_divisor != self.bloom_divisor {
            self.bloom_divisor = g.bloom_divisor;
            self.glow = glow_targets(self.grid, g.bloom_divisor);
        }
        let grid = vec2(self.grid.0 as f32, self.grid.1 as f32);
        let bloom = g.bloom && !heat_view;
        if bloom {
            self.emit.set_uniform("GridSize", grid);
            self.emit.set_uniform("Params", vec4(g.incandescence as u8 as f32, g.bloom_divisor as f32, 0.0, 0.0));
            pass(&self.emit, &self.cells, &self.glow[0]);
            let size = vec2(self.glow[0].texture.width(), self.glow[0].texture.height());
            self.blur.set_uniform("Step", vec2(1.0 / size.x, 0.0));
            let src = self.glow[0].texture.clone();
            pass(&self.blur, &src, &self.glow[1]);
            self.blur.set_uniform("Step", vec2(0.0, 1.0 / size.y));
            let src = self.glow[1].texture.clone();
            pass(&self.blur, &src, &self.glow[0]);
        }

        let flag = |b: bool| b as u8 as f32;
        self.scene.set_uniform("GridSize", grid);
        self.scene.set_uniform("Flags", vec4(flag(g.shading), flag(g.depth), flag(g.grain), flag(g.incandescence)));
        self.scene.set_uniform(
            "Flags2",
            vec4(if bloom { g.bloom_strength } else { 0.0 }, g.exposure, flag(g.vignette), flag(heat_view)),
        );
        self.scene.set_uniform("Time", time);
        self.scene.set_texture("Glow", self.glow[0].texture.clone());
        gl_use_material(&self.scene);
        draw_texture_ex(
            &self.cells,
            dest.x,
            dest.y,
            WHITE,
            DrawTextureParams { dest_size: Some(vec2(dest.w, dest.h)), ..Default::default() },
        );
        gl_use_default_material();
    }
}
