//! Thermal diffusion, phase changes, fire and lava: the rules and their CPU
//! reference. The WGSL in `shaders/thermal.wgsl`, `fire.wgsl` and `lava.wgsl`
//! must match [`ThermalReference`]; the per-material tables are generated from
//! this module into the shader headers, so both sides use identical numbers.
//!
//! A tick runs three passes in order: thermal, fire, lava.
//!
//! The GPU passes also count sound events (not modelled here): lava
//! freezing to rock and non-gunpowder ignition count as crackle, gunpowder
//! ignition as boom, and each lava move as flow.
//!
//! **Thermal** (double-buffered, every cell reads the state at pass start):
//! 1. Sanitize: a non-finite `temp` becomes [`AMBIENT_K`], a non-finite
//!    `fuel` becomes 0 (each counted); `temp` is clamped to `0..=MAX_TEMP_K`
//!    and `fuel` to `0..=MAX_TIMER`.
//! 2. Conduction over the 4 edges (outside the grid = insulated):
//!    `T += (sum over edges of edge_rate(m, n) * (T_n - T)) / C(m)`, where
//!    `edge_rate` is `DT` times the harmonic mean of the two
//!    [`conductivity`] values (series resistance; 0 when either side is 0)
//!    and `C` is the [`heat_capacity`]. The flux of an edge is computed
//!    identically from both sides with opposite sign, so heat (`sum C * T`)
//!    leaves one cell exactly as it enters the other.
//! 3. Moisture: a flammable cell with an orthogonal Water neighbour is wet;
//!    its temperature is capped at water's `boil_k`, and the excess heat
//!    `C * (T - cap)` is moved (not deleted) into its water neighbours, split
//!    equally. The water warms, boils and evaporates (by latent heat); once no water touches
//!    the cell it heats normally and can ignite.
//! 4. Phase changes on the new temperature, using `MATERIALS` thresholds:
//!    Ice -> Water at `melt_k + HYST_K`; Water -> Ice at `melt_k - HYST_K`;
//!    Water at or above `boil_k` is held at `boil_k` and stores the heat
//!    above it, `(T - boil_k) * C`, in `fuel` (latent heat absorbed); when
//!    `fuel >= LATENT_WATER` it becomes Steam. Below `boil_k`, stored latent
//!    heat is released first (up to `boil_k`). So heat is conserved as
//!    `sum C*T + sum water fuel`, and water's lifetime depends on its heat
//!    input; water does NOT use `evap_s`;
//!    Steam -> Water below `condense_k - HYST_K`;
//!    Lava -> Rock below [`LAVA_FREEZE_K`]; Stone and Rock -> Lava at their
//!    `melt_k`; Sand -> Glass at its `melt_k`. A change resets `fuel`.
//!
//! **Fire** (in place, each cell reads only itself):
//! * A flammable cell (finite `burn_s`, not Fire) at or above [`IGNITE_K`]
//!   becomes Fire with `fuel = ticks(burn_s)` of the original material and
//!   `temp = max(temp, FIRE_K)`.
//! * A Fire cell with `fuel < 1` (a freshly placed flame) gets
//!   `ticks(burn_s of Fire)`; otherwise `fuel -= 1`, and at 0 it becomes Ash
//!   (keeping its temperature). While burning, `temp = max(temp, FIRE_K)`.
//!   So ignited wood burns exactly `ticks(6.0)` = 360 ticks = 6 s.
//!
//! **Lava** (viscous liquid; `fuel` is its viscosity delay in ticks):
//! 1. Countdown (in place): `fuel = clamp(floor(fuel) - 1, 0, D - 1)` with
//!    `D = ticks(LAVA_DELAY_S)`.
//! 2. Two row-parity phases (`y % 2 == phase` acts; double-buffered). A lava
//!    cell acts only if `fuel < D` (so it moves at most once per tick). It
//!    falls straight down into an empty cell at once; otherwise, only when
//!    `fuel < 1`, it tries diagonal-down (hash bit 0 picks the side) and,
//!    only with lava directly above it (a yield stress: a one-cell-thick
//!    sheet stops spreading), one cell sideways (hash bit 1). A move swaps the
//!    lava with the empty cell (the air keeps its own temperature and vapor)
//!    and sets the lava's `fuel = D`. Conflicts for one empty cell: above,
//!    above-left, above-right, left, right. Outside the grid is solid.
//!
//! Heat capacity is per cell (all cells have equal volume), relative to
//! granite. Air's real value (~6e-4) is raised to 0.01 so the explicit step
//! stays stable; lava's includes the latent heat of crystallisation spread
//! over its liquid range. Conductivities are a hand-compressed scale that
//! keeps the real ordering (air 0.02, stone 0.2, metal 0.5 per the plan).
//! Units: conductivity in 1/s per edge, so rates do not depend on the tick.

use crate::layout::CellRaw;
use crate::materials::{Material, MATERIALS, MATERIAL_COUNT, WALL};
use crate::reference::cell_hash;
use crate::units::{ticks, DT};

/// Ambient temperature in kelvin (non-finite temperatures reset to this).
pub const AMBIENT_K: f32 = crate::air::AMBIENT_K;
/// Upper clamp for temperatures, kelvin.
pub const MAX_TEMP_K: f32 = 10_000.0;
/// Upper clamp for the per-cell timer (`fuel`), ticks.
pub const MAX_TIMER: f32 = 1.0e6;
/// Ignition temperature of flammable materials, kelvin. Wood autoignition is
/// about 573 K (300 C; Babrauskas, "Ignition Handbook", 2003, wood 250-400 C).
pub const IGNITE_K: f32 = 573.0;
/// Temperature a burning cell holds itself at, kelvin: a wood fire's flame
/// is about 1100-1300 K (Drysdale, "An Introduction to Fire Dynamics", 3rd ed.).
pub const FIRE_K: f32 = 1200.0;
/// Lava solidifies to Rock below this, kelvin. A GAMEPLAY value carried over
/// from v1: real basalt solidifies near 1250 K.
pub const LAVA_FREEZE_K: f32 = 700.0;
/// Hysteresis around melting and condensation points, kelvin (no flicker).
pub const HYST_K: f32 = 2.0;
/// Viscosity delay of lava in seconds: after a move, lava waits this long
/// before it may slide or spread again (falling is never delayed).
pub const LAVA_DELAY_S: f32 = 0.25;
/// Latent heat of vaporisation of one water cell, in heat-capacity units x K
/// (relative to granite like [`heat_capacity`]): 2.257 MJ/kg x 1000 kg/m^3 /
/// 2.13 MJ/(m^3 K) for granite = 1060. Boiling water absorbs heat into `fuel`
/// until this budget is spent, then becomes Steam.
pub const LATENT_WATER: f32 = 1060.0;
/// Number of entries in the per-material tables: every material plus [`WALL`].
pub const TABLE_N: usize = MATERIAL_COUNT + 1;

const EMPTY: u32 = Material::Empty as u32;
const WATER: u32 = Material::Water as u32;
const LAVA: u32 = Material::Lava as u32;
const FIRE: u32 = Material::Fire as u32;

/// Table index of a material id; unknown ids act as [`WALL`].
fn tid(mat: u32) -> usize {
    mat.min(WALL) as usize
}

/// Thermal conductivity of `mat` in 1/s per edge (0 = insulating). Fire's
/// value stands for flame convection and radiation, not conduction.
pub fn conductivity(mat: u32) -> f32 {
    use Material::*;
    const K: [f32; TABLE_N] = {
        let mut k = [0.0f32; TABLE_N];
        k[Empty as usize] = 0.02;
        k[Sand as usize] = 0.05;
        k[Water as usize] = 0.08;
        k[Oil as usize] = 0.04;
        k[Stone as usize] = 0.2;
        k[Rock as usize] = 0.15;
        k[Wood as usize] = 0.05;
        k[Ash as usize] = 0.04;
        k[Metal as usize] = 0.5;
        k[Glass as usize] = 0.09;
        k[Lava as usize] = 0.1;
        k[Acid as usize] = 0.08;
        k[Gunpowder as usize] = 0.04;
        k[Ice as usize] = 0.15;
        k[Fire as usize] = 0.5;
        k[Steam as usize] = 0.02;
        k[Smoke as usize] = 0.02;
        k[Ember as usize] = 0.05;
        k // WALL stays 0: the boundary is insulating
    };
    K[tid(mat)]
}

/// Volumetric heat capacity of `mat` relative to granite (density times
/// specific heat; equal cell volumes).
pub fn heat_capacity(mat: u32) -> f32 {
    use Material::*;
    const C: [f32; TABLE_N] = {
        let mut c = [1.0f32; TABLE_N];
        c[Empty as usize] = 0.01;
        c[Sand as usize] = 0.6;
        c[Water as usize] = 2.0;
        c[Oil as usize] = 0.75;
        c[Stone as usize] = 1.0;
        c[Rock as usize] = 1.2;
        c[Wood as usize] = 0.5;
        c[Ash as usize] = 0.25;
        c[Metal as usize] = 1.7;
        c[Glass as usize] = 1.0;
        c[Lava as usize] = 2.2;
        c[Acid as usize] = 0.8;
        c[Gunpowder as usize] = 0.5;
        c[Ice as usize] = 0.9;
        c[Fire as usize] = 0.1;
        c[Steam as usize] = 0.01;
        c[Smoke as usize] = 0.01;
        c[Ember as usize] = 0.7;
        c // WALL = 1 (it never exchanges heat)
    };
    C[tid(mat)]
}

/// Fraction of the temperature difference exchanged across an edge between
/// `a` and `b` per tick: `DT * 2 ka kb / (ka + kb)`. Symmetric bit for bit.
pub fn edge_rate(a: u32, b: u32) -> f32 {
    let (ka, kb) = (conductivity(a), conductivity(b));
    if ka <= 0.0 || kb <= 0.0 {
        return 0.0;
    }
    DT * (2.0 * ka * kb / (ka + kb))
}

/// Heat flowing into cell `a` (temperature `ta`) from neighbour `b` across one
/// edge in one tick. `edge_flux(a, ta, b, tb) == -edge_flux(b, tb, a, ta)` exactly.
pub fn edge_flux(a: u32, ta: f32, b: u32, tb: f32) -> f32 {
    edge_rate(a, b) * (tb - ta)
}

/// Worst-case explicit stability number over all materials: four edges at
/// the largest rate, divided by the heat capacity. Must be at most 0.5.
pub fn stability_number() -> f32 {
    (0..=WALL)
        .map(|a| {
            let worst = (0..=WALL).map(|b| edge_rate(a, b)).fold(0.0, f32::max);
            4.0 * worst / heat_capacity(a)
        })
        .fold(0.0, f32::max)
}

/// True if `mat` can ignite: finite `burn_s` and not already Fire.
pub fn is_flammable(mat: u32) -> bool {
    (mat as usize) < MATERIAL_COUNT && mat != FIRE && MATERIALS[mat as usize].burn_s.is_finite()
}

/// Burn time of `mat` in ticks (0 when it does not burn).
pub fn burn_ticks(mat: u32) -> u32 {
    match MATERIALS.get(mat as usize) {
        Some(d) if d.burn_s.is_finite() => ticks(d.burn_s),
        _ => 0,
    }
}

/// Lava viscosity delay in ticks (`ticks(LAVA_DELAY_S)`).
pub fn lava_delay_ticks() -> u32 {
    ticks(LAVA_DELAY_S)
}

/// Phase-change thresholds, derived once from `MATERIALS`.
struct Thresholds {
    ice_melt: f32,
    water_freeze: f32,
    water_boil: f32,
    steam_condense: f32,
    stone_melt: f32,
    rock_melt: f32,
    sand_melt: f32,
}

fn thresholds() -> Thresholds {
    let m = |x: Material| &MATERIALS[x as usize];
    Thresholds {
        ice_melt: m(Material::Ice).melt_k + HYST_K,
        water_freeze: m(Material::Water).melt_k - HYST_K,
        water_boil: m(Material::Water).boil_k,
        steam_condense: m(Material::Steam).condense_k - HYST_K,
        stone_melt: m(Material::Stone).melt_k,
        rock_melt: m(Material::Rock).melt_k,
        sand_melt: m(Material::Sand).melt_k,
    }
}

fn wgsl_f32(name: &str, v: f32) -> String {
    assert!(v.is_finite(), "{name} must be finite");
    format!("const {name}: f32 = {v:?}f;\n")
}

fn wgsl_bits(name: &str, vals: impl Iterator<Item = u32>) -> String {
    let v: Vec<String> = vals.map(|b| format!("{b:#x}u")).collect();
    format!(
        "var<private> {name}: array<u32, {n}> = array<u32, {n}>({});\n",
        v.join(", "),
        n = v.len()
    )
}

/// WGSL header shared by the thermal, fire and lava shaders: material ids,
/// thresholds and the per-material tables (as exact f32 bit patterns).
pub(crate) fn wgsl_header() -> String {
    use Material::*;
    let th = thresholds();
    let mut s = String::new();
    for (name, m) in [
        ("EMPTY", Empty),
        ("WATER", Water),
        ("ICE", Ice),
        ("STEAM", Steam),
        ("LAVA", Lava),
        ("ROCK", Rock),
        ("STONE", Stone),
        ("SAND", Sand),
        ("GLASS", Glass),
        ("FIRE", Fire),
        ("ASH", Ash),
        ("GUNPOWDER", Gunpowder),
    ] {
        s += &format!("const {name}: u32 = {}u;\n", m as u32);
    }
    s += &format!("const WALL: u32 = {WALL}u;\nconst TABLE_N: u32 = {TABLE_N}u;\n");
    for (name, v) in [
        ("AMBIENT_K", AMBIENT_K),
        ("MAX_TEMP_K", MAX_TEMP_K),
        ("MAX_TIMER", MAX_TIMER),
        ("IGNITE_K", IGNITE_K),
        ("FIRE_K", FIRE_K),
        ("LAVA_FREEZE_K", LAVA_FREEZE_K),
        ("ICE_MELT_K", th.ice_melt),
        ("WATER_FREEZE_K", th.water_freeze),
        ("WATER_BOIL_K", th.water_boil),
        ("STEAM_CONDENSE_K", th.steam_condense),
        ("STONE_MELT_K", th.stone_melt),
        ("ROCK_MELT_K", th.rock_melt),
        ("SAND_MELT_K", th.sand_melt),
        ("LATENT_WATER", LATENT_WATER),
        ("LAVA_DELAY", lava_delay_ticks() as f32),
        ("FIRE_TICKS", burn_ticks(FIRE) as f32),
    ] {
        s += &wgsl_f32(name, v);
    }
    let n = TABLE_N as u32;
    s += &wgsl_bits(
        "EDGE_BITS",
        (0..n * n).map(|i| edge_rate(i / n, i % n).to_bits()),
    );
    s += &wgsl_bits("C_BITS", (0..n).map(|m| heat_capacity(m).to_bits()));
    s += &wgsl_bits(
        "INV_C_BITS",
        (0..n).map(|m| (1.0 / heat_capacity(m)).to_bits()),
    );
    s += &wgsl_bits(
        "BURN_TICKS",
        (0..n).map(|m| if is_flammable(m) { burn_ticks(m) } else { 0 }),
    );
    s
}

/// CPU model of the thermal, fire and lava passes on full [`CellRaw`] cells.
pub struct ThermalReference {
    w: u32,
    h: u32,
    /// Row-major cells; tests set up scenes here directly.
    pub cells: Vec<CellRaw>,
    /// Non-finite `temp`/`fuel` values reset so far (mirrors the GPU counter).
    pub sanitized: u32,
}

fn san_temp(t: f32) -> f32 {
    if t.is_finite() {
        t.clamp(0.0, MAX_TEMP_K)
    } else {
        AMBIENT_K
    }
}

impl ThermalReference {
    /// A `w` x `h` grid of air at [`AMBIENT_K`].
    pub fn new(w: u32, h: u32) -> ThermalReference {
        let air = CellRaw {
            mat: EMPTY,
            temp: AMBIENT_K,
            ..Default::default()
        };
        ThermalReference {
            w,
            h,
            cells: vec![air; (w * h) as usize],
            sanitized: 0,
        }
    }

    fn inside(&self, x: i64, y: i64) -> bool {
        x >= 0 && y >= 0 && x < i64::from(self.w) && y < i64::from(self.h)
    }

    fn ix(&self, x: i64, y: i64) -> usize {
        (y * i64::from(self.w) + x) as usize
    }

    /// One full tick: thermal, fire, lava.
    pub fn tick(&mut self, tick: u32, seed: u32) {
        self.thermal();
        self.fire();
        self.lava(tick, seed);
    }

    /// Sanitized temperature of (`x`, `y`) after conduction, and its number
    /// of orthogonal Water neighbours.
    fn conducted(&self, old: &[CellRaw], x: i64, y: i64) -> (f32, u32) {
        let c = old[self.ix(x, y)];
        let t = san_temp(c.temp);
        let (mut acc, mut waters) = (0.0f32, 0u32);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            if !self.inside(x + dx, y + dy) {
                continue;
            }
            let n = old[self.ix(x + dx, y + dy)];
            acc += edge_flux(c.mat, t, n.mat, san_temp(n.temp));
            waters += u32::from(n.mat == WATER);
        }
        (t + acc * (1.0 / heat_capacity(c.mat)), waters)
    }

    /// Temperature rise of the water cell (`x`, `y`) from the heat its wet
    /// flammable neighbours shed at the moisture cap: each neighbour's excess
    /// `C * (T - cap)` is split equally between its water neighbours.
    fn moisture_heat(&self, old: &[CellRaw], x: i64, y: i64, cap: f32) -> f32 {
        let mut gain = 0.0f32;
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if !self.inside(nx, ny) || !is_flammable(old[self.ix(nx, ny)].mat) {
                continue;
            }
            let (tn, wn) = self.conducted(old, nx, ny);
            if tn > cap {
                let excess = (tn - cap) * heat_capacity(old[self.ix(nx, ny)].mat);
                gain += excess / wn as f32;
            }
        }
        gain * (1.0 / heat_capacity(WATER))
    }

    /// The thermal pass: sanitize, conduct, moisture cap, phase changes.
    pub fn thermal(&mut self) {
        let th = thresholds();
        let old = self.cells.clone();
        for y in 0..i64::from(self.h) {
            for x in 0..i64::from(self.w) {
                let i = self.ix(x, y);
                let c = old[i];
                let m = c.mat;
                if !c.temp.is_finite() {
                    self.sanitized += 1;
                }
                if !c.fuel.is_finite() {
                    self.sanitized += 1;
                }
                let mut f = if c.fuel.is_finite() {
                    c.fuel.clamp(0.0, MAX_TIMER)
                } else {
                    0.0
                };
                let (mut t, waters) = self.conducted(&old, x, y);
                if waters > 0 && is_flammable(m) {
                    t = t.min(th.water_boil);
                }
                if m == WATER {
                    t += self.moisture_heat(&old, x, y, th.water_boil);
                }
                let mut mat = m;
                use Material::*;
                match m {
                    _ if m == Ice as u32 && t >= th.ice_melt => mat = Water as u32,
                    _ if m == Water as u32 => {
                        let cw = heat_capacity(WATER);
                        if t >= th.water_boil {
                            f += (t - th.water_boil) * cw;
                            t = th.water_boil;
                            if f >= LATENT_WATER {
                                mat = Steam as u32;
                            }
                        } else if f > 0.0 {
                            let r = f.min((th.water_boil - t) * cw);
                            t += r * (1.0 / cw);
                            f -= r;
                        }
                        if mat == m && t <= th.water_freeze {
                            mat = Ice as u32;
                        }
                    }
                    _ if m == Steam as u32 && t < th.steam_condense => mat = Water as u32,
                    _ if m == Lava as u32 && t < LAVA_FREEZE_K => mat = Rock as u32,
                    _ if m == Stone as u32 && t >= th.stone_melt => mat = Lava as u32,
                    _ if m == Rock as u32 && t >= th.rock_melt => mat = Lava as u32,
                    _ if m == Sand as u32 && t >= th.sand_melt => mat = Glass as u32,
                    _ => {}
                }
                if mat != m {
                    f = 0.0;
                }
                self.cells[i] = CellRaw {
                    mat,
                    temp: t,
                    fuel: f,
                    ..c
                };
            }
        }
    }

    /// The fire pass: ignition and burn countdown.
    pub fn fire(&mut self) {
        for c in &mut self.cells {
            if is_flammable(c.mat) && c.temp >= IGNITE_K {
                c.fuel = burn_ticks(c.mat) as f32;
                c.mat = FIRE;
                c.temp = c.temp.max(FIRE_K);
            } else if c.mat == FIRE {
                if c.fuel.is_nan() || c.fuel < 1.0 {
                    c.fuel = burn_ticks(FIRE) as f32;
                    c.temp = c.temp.max(FIRE_K);
                } else {
                    c.fuel -= 1.0;
                    if c.fuel < 1.0 {
                        c.mat = Material::Ash as u32;
                        c.fuel = 0.0;
                    } else {
                        c.temp = c.temp.max(FIRE_K);
                    }
                }
            }
        }
    }

    /// The lava pass for `tick`: countdown, then the two row phases.
    pub fn lava(&mut self, tick: u32, seed: u32) {
        let d = lava_delay_ticks() as f32;
        for c in &mut self.cells {
            if c.mat == LAVA {
                let f = if c.fuel.is_finite() {
                    c.fuel.floor()
                } else {
                    0.0
                };
                c.fuel = (f - 1.0).max(0.0).min(d - 1.0);
            }
        }
        for phase in 0..2 {
            self.lava_phase(phase, tick, seed, d);
        }
    }

    fn mat_at(&self, x: i64, y: i64) -> u32 {
        if self.inside(x, y) {
            self.cells[self.ix(x, y)].mat
        } else {
            WALL
        }
    }

    fn empty(&self, x: i64, y: i64) -> bool {
        self.mat_at(x, y) == EMPTY
    }

    /// Target of the lava at (`x`, `y`) this phase, if it acts and wants to move.
    fn lava_plan(
        &self,
        phase: u32,
        tick: u32,
        seed: u32,
        d: f32,
        x: i64,
        y: i64,
    ) -> Option<(i64, i64)> {
        if !self.inside(x, y) || y as u32 % 2 != phase || self.mat_at(x, y) != LAVA {
            return None;
        }
        let c = self.cells[self.ix(x, y)];
        if c.fuel.is_nan() || c.fuel >= d {
            return None;
        }
        if self.empty(x, y + 1) {
            return Some((x, y + 1));
        }
        if c.fuel.is_nan() || c.fuel >= 1.0 {
            return None;
        }
        let h = cell_hash(tick, x as u32, y as u32, seed);
        let s = if h & 1 == 1 { 1 } else { -1 };
        for dx in [s, -s] {
            if self.empty(x + dx, y + 1) {
                return Some((x + dx, y + 1));
            }
        }
        if self.mat_at(x, y - 1) == LAVA {
            let s = if (h >> 1) & 1 == 1 { 1 } else { -1 };
            for dx in [s, -s] {
                if self.empty(x + dx, y) {
                    return Some((x + dx, y));
                }
            }
        }
        None
    }

    /// Lava cell that wins the empty cell (`x`, `y`), by fixed priority.
    fn lava_winner(
        &self,
        phase: u32,
        tick: u32,
        seed: u32,
        d: f32,
        x: i64,
        y: i64,
    ) -> Option<(i64, i64)> {
        [
            (x, y - 1),
            (x - 1, y - 1),
            (x + 1, y - 1),
            (x - 1, y),
            (x + 1, y),
        ]
        .into_iter()
        .find(|&(sx, sy)| self.lava_plan(phase, tick, seed, d, sx, sy) == Some((x, y)))
    }

    fn lava_phase(&mut self, phase: u32, tick: u32, seed: u32, d: f32) {
        let mut out = self.cells.clone();
        for y in 0..i64::from(self.h) {
            for x in 0..i64::from(self.w) {
                let i = self.ix(x, y);
                let c = self.cells[i];
                if c.mat == EMPTY {
                    if let Some((sx, sy)) = self.lava_winner(phase, tick, seed, d, x, y) {
                        out[i] = CellRaw {
                            fuel: d,
                            ..self.cells[self.ix(sx, sy)]
                        };
                    }
                } else if let Some((tx, ty)) = self.lava_plan(phase, tick, seed, d, x, y) {
                    if self.lava_winner(phase, tick, seed, d, tx, ty) == Some((x, y)) {
                        out[i] = self.cells[self.ix(tx, ty)];
                    }
                }
            }
        }
        self.cells = out;
    }
}
