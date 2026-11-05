// Per-cell colour (cell resolution, HDR): material albedo with hash noise,
// lit by AMBIENT + LIGHT_GAIN * light; liquids darken with depth
// (Beer-Lambert, per-channel ABSORB) and their top cell gets a highlight
// from the surface slope of the neighbouring columns; gases (empty, steam,
// smoke, fire) show the background plus light scattered in air, overlaid by
// vapor/steam/smoke with alpha 1 - exp(-VAPOR_K * vapor) (Steam/Smoke cells
// at least STEAM_ALPHA/SMOKE_ALPHA), blurred 3x3 for soft edges and varied
// by noise (0.85..1.15); hot cells add blackbody emission. Solid faces are
// lit because the light resolve marches from the solid cell itself.
//
// Bindings: 0 view uniform, 1 cells (storage, read), 2 light texture,
// 3 output (storage texture, write).

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<storage, read> cells: array<Cell>;
@group(0) @binding(2) var light: texture_2d<f32>;
@group(0) @binding(3) var out_col: texture_storage_2d<rgba16float, write>;

fn mat_at(x: i32, y: i32) -> u32 {
    if x < 0 || y < 0 || x >= i32(view.w) || y >= i32(view.h) {
        return WALL;
    }
    return min(cells[u32(y) * view.w + u32(x)].mat, WALL);
}

fn is_gas(m: u32) -> bool {
    return m == EMPTY || m == STEAM || m == SMOKE || m == FIRE;
}

// Gas of one cell, premultiplied: rgb = colour * alpha, a = opacity from
// vapor, with a floor for Steam and Smoke cells.
fn gas(x: i32, y: i32) -> vec4<f32> {
    if x < 0 || y < 0 || x >= i32(view.w) || y >= i32(view.h) {
        return vec4<f32>(0.0);
    }
    let c = cells[u32(y) * view.w + u32(x)];
    var vap = 0.0;
    if finite(c.vapor) {
        vap = clamp(c.vapor, 0.0, 100.0);
    }
    var a = 1.0 - exp(-VAPOR_K * vap);
    var rgb = STEAM_RGB;
    let m = min(c.mat, WALL);
    if m == STEAM {
        a = max(a, STEAM_ALPHA);
    } else if m == SMOKE {
        a = max(a, SMOKE_ALPHA);
        rgb = SMOKE_RGB;
    }
    return vec4<f32>(rgb * a, a);
}

// 3x3 binomial blur of premultiplied gas, so smoke and steam have soft edges.
fn soft_gas(x: i32, y: i32) -> vec4<f32> {
    var g = 4.0 * gas(x, y);
    g += 2.0 * (gas(x + 1, y) + gas(x - 1, y) + gas(x, y + 1) + gas(x, y - 1));
    g += gas(x + 1, y + 1) + gas(x - 1, y + 1) + gas(x + 1, y - 1) + gas(x - 1, y - 1);
    return g / 16.0;
}

// Height (cells, up is positive) of the top of liquid `m` in column `x`,
// relative to row `y`. A solid column counts as level (0).
fn surface_height(x: i32, y: i32, m: u32) -> f32 {
    let here = mat_at(x, y);
    if here == m {
        var k = 0;
        while k < SURF_SEARCH && mat_at(x, y - k - 1) == m {
            k++;
        }
        return f32(k);
    }
    if !is_gas(here) {
        return 0.0;
    }
    for (var k = 1; k <= SURF_SEARCH; k++) {
        if mat_at(x, y + k) == m {
            return -f32(k);
        }
    }
    return -f32(SURF_SEARCH);
}

@compute @workgroup_size(8, 8)
fn shade(@builtin(global_invocation_id) g: vec3<u32>) {
    if g.x >= view.w || g.y >= view.h {
        return;
    }
    let x = i32(g.x);
    let y = i32(g.y);
    let c = cells[g.y * view.w + g.x];
    let m = min(c.mat, WALL);
    let t = safe_temp(c.temp);
    let n = hash01(g.x, g.y);
    let lit = textureLoad(light, vec2<i32>(x, y), 0).rgb;
    let illum = AMBIENT + LIGHT_GAIN * lit;
    var col: vec3<f32>;
    if is_gas(m) {
        let bg = mix(BG_TOP, BG_BOTTOM, f32(y) / f32(view.h));
        col = bg + AIR_GLOW * lit;
        let sg = soft_gas(x, y);
        let a = clamp(sg.a * (0.85 + 0.3 * n), 0.0, 1.0);
        let tint = sg.rgb / max(sg.a, 1e-4);
        col = mix(col, tint * illum, a);
    } else {
        let alb = albedo(m);
        var base = alb.rgb * (1.0 + alb.w * (2.0 * n - 1.0));
        var spec = vec3<f32>(0.0);
        if is_liquid(m) {
            var d = 0;
            while d < DEPTH_MAX && mat_at(x, y - d - 1) == m {
                d++;
            }
            var absorb = ABSORB_OTHER;
            if m == WATER {
                absorb = ABSORB_WATER;
            }
            base *= exp(-absorb * f32(d));
            if d == 0 {
                let slope = 0.5 * (surface_height(x + 1, y, m) - surface_height(x - 1, y, m));
                let nrm = normalize(vec2<f32>(-slope, -1.0));
                spec = SURF_RGB * SURF_SPEC * pow(max(dot(nrm, SUN_DIR), 0.0), SURF_POW);
            }
        }
        col = base * illum + spec * illum;
    }
    var e = blackbody(t);
    if m == FIRE {
        e *= FIRE_FLICKER + (1.0 - FIRE_FLICKER) * n;
    }
    col += e;
    textureStore(out_col, vec2<i32>(x, y), vec4<f32>(finite3(col), 1.0));
}
