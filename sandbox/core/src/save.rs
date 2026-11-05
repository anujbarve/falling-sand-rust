//! Versioned plain-text save and load of the grid and air layers.
//!
//! Format: `version 1`, `size W H`, then one `mat temp fuel` line per cell, then
//! one `vx vy pressure vapor` line per cell. Particles, events, waves and light
//! are transient and are not saved (waves and light are recomputed by systems).

use crate::material::Material;
use crate::vec2::Vec2;
use crate::world::World;
use std::fmt;
use std::fmt::Write as _;
use std::path::Path;

const VERSION: u32 = 1;
/// Largest accepted width or height, in cells.
const MAX_DIM: usize = 4096;
const MAX_CELLS: usize = 1 << 24;

const MATERIALS: [Material; 18] = [
    Material::Empty,
    Material::Sand,
    Material::Water,
    Material::Oil,
    Material::Stone,
    Material::Rock,
    Material::Wood,
    Material::Ash,
    Material::Metal,
    Material::Glass,
    Material::Lava,
    Material::Acid,
    Material::Gunpowder,
    Material::Ice,
    Material::Fire,
    Material::Steam,
    Material::Smoke,
    Material::Ember,
];

/// Why a load failed.
#[derive(Debug)]
pub enum SaveError {
    /// The file could not be read.
    Io(std::io::Error),
    /// The file declares a version this build does not understand.
    BadVersion(u32),
    /// The file is truncated or malformed; the message says where.
    Corrupt(String),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Io(e) => write!(f, "save io error: {e}"),
            SaveError::BadVersion(v) => write!(f, "unsupported save version {v}"),
            SaveError::Corrupt(m) => write!(f, "corrupt save: {m}"),
        }
    }
}

impl std::error::Error for SaveError {}

/// Writes `w` to `path` atomically: the text goes to `path` + `.tmp`, then is
/// renamed over `path`, so a failure leaves any existing file untouched.
pub fn save_world(w: &World, path: &Path) -> std::io::Result<()> {
    let mut s = String::new();
    let _ = writeln!(s, "version {VERSION}");
    let _ = writeln!(s, "size {} {}", w.cells.width(), w.cells.height());
    for c in w.cells.as_slice() {
        let _ = writeln!(s, "{} {} {}", c.mat as u8, c.temp, c.fuel);
    }
    for i in 0..w.air.vel.len() {
        let v = w.air.vel[i];
        let _ = writeln!(s, "{} {} {} {}", v.x, v.y, w.air.pressure[i], w.air.vapor[i]);
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    if let Err(e) = std::fs::write(&tmp, s).and_then(|_| std::fs::rename(&tmp, path)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

fn bad(m: impl Into<String>) -> SaveError {
    SaveError::Corrupt(m.into())
}

fn num<T: std::str::FromStr>(tok: Option<&str>, what: &str, line: usize) -> Result<T, SaveError> {
    tok.ok_or_else(|| bad(format!("line {line}: missing {what}")))?
        .parse()
        .map_err(|_| bad(format!("line {line}: bad {what}")))
}

fn fnum(tok: Option<&str>, what: &str, line: usize) -> Result<f32, SaveError> {
    let v: f32 = num(tok, what, line)?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(bad(format!("line {line}: non-finite {what}")))
    }
}

/// Reads a world saved by [`save_world`]. Width and height must each be 1..=4096 and all floats finite. Unknown versions give
/// `BadVersion`; truncated or malformed content gives `Corrupt`; never panics.
pub fn load_world(path: &Path) -> Result<World, SaveError> {
    let text = std::fs::read_to_string(path).map_err(SaveError::Io)?;
    let mut lines = text.lines().enumerate().map(|(i, l)| (i + 1, l));

    let (n, first) = lines.next().ok_or_else(|| bad("empty file"))?;
    let mut t = first.split_whitespace();
    if t.next() != Some("version") {
        return Err(bad("line 1: expected `version N`"));
    }
    let ver: u32 = num(t.next(), "version", n)?;
    if ver != VERSION {
        return Err(SaveError::BadVersion(ver));
    }

    let (n, size) = lines.next().ok_or_else(|| bad("missing size line"))?;
    let mut t = size.split_whitespace();
    if t.next() != Some("size") {
        return Err(bad("line 2: expected `size W H`"));
    }
    let (w, h): (usize, usize) = (num(t.next(), "width", n)?, num(t.next(), "height", n)?);
    if w == 0 || h == 0 || w > MAX_DIM || h > MAX_DIM {
        return Err(bad(format!("line {n}: size must be 1..={MAX_DIM} per side")));
    }
    let cells = w * h;
    if cells > MAX_CELLS {
        return Err(bad("size too large"));
    }

    let mut world = World::new(w, h);
    for c in world.cells.as_mut_slice().iter_mut() {
        let (n, l) = lines.next().ok_or_else(|| bad("truncated in cell data"))?;
        let mut t = l.split_whitespace();
        let m: usize = num(t.next(), "material", n)?;
        c.mat = *MATERIALS.get(m).ok_or_else(|| bad(format!("line {n}: unknown material {m}")))?;
        c.temp = fnum(t.next(), "temp", n)?;
        c.fuel = num(t.next(), "fuel", n)?;
    }
    for i in 0..cells {
        let (n, l) = lines.next().ok_or_else(|| bad("truncated in air data"))?;
        let mut t = l.split_whitespace();
        world.air.vel[i] = Vec2 { x: fnum(t.next(), "vx", n)?, y: fnum(t.next(), "vy", n)? };
        world.air.pressure[i] = fnum(t.next(), "pressure", n)?;
        world.air.vapor[i] = fnum(t.next(), "vapor", n)?;
    }
    Ok(world)
}
