//! Keyboard mapping, control state and brush strokes (v1 mapping). Pure, so
//! it is testable without a window.

use sandbox_v2_gpu::materials::Material;
use sandbox_v2_gpu::reference_thermal::{AMBIENT_K, FIRE_K};
use sandbox_v2_gpu::Stroke;
use winit::keyboard::{Key, NamedKey};

use crate::scenes::LAVA_K;

/// Largest brush radius in cells.
pub const MAX_BRUSH: u32 = 20;

/// Materials on keys 1 to 8, in key order (v1 mapping).
pub const KEY_MATERIALS: [Material; 8] = [
    Material::Sand,
    Material::Water,
    Material::Oil,
    Material::Stone,
    Material::Wood,
    Material::Gunpowder,
    Material::Fire,
    Material::Lava,
];

/// What a key press asks the app to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Paint with this material from now on.
    SelectMaterial(Material),
    /// Grow the brush by one cell (`]`).
    BrushLarger,
    /// Shrink the brush by one cell (`[`).
    BrushSmaller,
    /// Save the world (`S`).
    Save,
    /// Load the saved world (`L`).
    Load,
    /// Pause or resume the simulation (`Space`).
    TogglePause,
    /// Leave the app (`Escape`).
    Quit,
}

/// Maps a logical key to its action; other keys map to `None`.
pub fn map_key(key: &Key) -> Option<Action> {
    match key {
        Key::Named(NamedKey::Space) => Some(Action::TogglePause),
        Key::Named(NamedKey::Escape) => Some(Action::Quit),
        Key::Character(s) => match s.as_str() {
            "[" => Some(Action::BrushSmaller),
            "]" => Some(Action::BrushLarger),
            "s" | "S" => Some(Action::Save),
            "l" | "L" => Some(Action::Load),
            d => {
                let n: usize = d.parse().ok()?;
                (1..=8)
                    .contains(&n)
                    .then(|| Action::SelectMaterial(KEY_MATERIALS[n - 1]))
            }
        },
        _ => None,
    }
}

/// The user-controlled state: selected material, brush radius, pause and quit.
#[derive(Clone, Copy, Debug)]
pub struct Controls {
    /// Material painted by the mouse.
    pub material: Material,
    /// Brush radius in cells, `1..=MAX_BRUSH`.
    pub brush: u32,
    /// Whether the simulation is paused.
    pub paused: bool,
    /// Set when the user asked to quit.
    pub quit: bool,
}

impl Controls {
    /// Sand selected, brush radius `brush` clamped to `1..=MAX_BRUSH`.
    pub fn new(brush: u32) -> Controls {
        Controls {
            material: Material::Sand,
            brush: brush.clamp(1, MAX_BRUSH),
            paused: false,
            quit: false,
        }
    }

    /// Applies an action. `Save` and `Load` need file IO and are returned
    /// for the caller; everything else returns `None`.
    pub fn apply(&mut self, action: Action) -> Option<Action> {
        match action {
            Action::SelectMaterial(m) => self.material = m,
            Action::BrushLarger => self.brush = (self.brush + 1).min(MAX_BRUSH),
            Action::BrushSmaller => self.brush = self.brush.saturating_sub(1).max(1),
            Action::TogglePause => self.paused = !self.paused,
            Action::Quit => self.quit = true,
            Action::Save | Action::Load => return Some(action),
        }
        None
    }
}

/// A stroke of `mat` centred on cell `(x, y)`: lava at [`LAVA_K`], fire at
/// `FIRE_K` with timer 0 (the fire pass gives it a flame's burn time),
/// everything else at ambient.
pub fn stroke(mat: Material, x: i32, y: i32, radius: u32) -> Stroke {
    let temp = match mat {
        Material::Lava => LAVA_K,
        Material::Fire => FIRE_K,
        _ => AMBIENT_K,
    };
    Stroke {
        cx: x,
        cy: y,
        r: radius as i32,
        mat: mat as u32,
        temp,
        fuel: 0.0,
        _pad: [0; 2],
    }
}

/// Grid cell under window pixel `(px, py)` for a `win_w` x `win_h` window
/// showing a `grid_w` x `grid_h` grid stretched to fill it. `None` outside
/// the window or for a zero-size window.
pub fn cell_at(px: f64, py: f64, win: (u32, u32), grid: (u32, u32)) -> Option<(i32, i32)> {
    let (ww, wh) = (f64::from(win.0), f64::from(win.1));
    if ww <= 0.0 || wh <= 0.0 || px < 0.0 || py < 0.0 || px >= ww || py >= wh {
        return None;
    }
    Some((
        (px / ww * f64::from(grid.0)) as i32,
        (py / wh * f64::from(grid.1)) as i32,
    ))
}
