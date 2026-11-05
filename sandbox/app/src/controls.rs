//! Keyboard mapping, control state and brush painting. Everything here is pure, so it is
//! testable without a window.

use macroquad::prelude::KeyCode;
use sandbox_core::material::Material;
use sandbox_core::world::World;
use sandbox_systems::fire::DEFAULT_FUEL;
use sandbox_systems::volcano::LAVA_TEMP;

/// Largest brush radius in cells.
pub const MAX_BRUSH: u32 = 20;

/// Materials on keys 1 to 8, in key order.
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

/// Maps a key to its action; keys 1 to 8 select [`KEY_MATERIALS`]. Other keys map to `None`.
pub fn map_key(key: KeyCode) -> Option<Action> {
    let keys = [
        KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4,
        KeyCode::Key5, KeyCode::Key6, KeyCode::Key7, KeyCode::Key8,
    ];
    if let Some(i) = keys.iter().position(|&k| k == key) {
        return Some(Action::SelectMaterial(KEY_MATERIALS[i]));
    }
    match key {
        KeyCode::LeftBracket => Some(Action::BrushSmaller),
        KeyCode::RightBracket => Some(Action::BrushLarger),
        KeyCode::S => Some(Action::Save),
        KeyCode::L => Some(Action::Load),
        KeyCode::Space => Some(Action::TogglePause),
        KeyCode::Escape => Some(Action::Quit),
        _ => None,
    }
}

/// Every key the app polls, for the main loop.
pub const POLLED_KEYS: [KeyCode; 14] = [
    KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4,
    KeyCode::Key5, KeyCode::Key6, KeyCode::Key7, KeyCode::Key8,
    KeyCode::LeftBracket, KeyCode::RightBracket, KeyCode::S, KeyCode::L,
    KeyCode::Space, KeyCode::Escape,
];

/// The user-controlled state: selected material, brush radius, pause and quit flags.
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
    /// Starts with sand selected and the given brush radius (clamped to `1..=MAX_BRUSH`).
    pub fn new(brush: u32) -> Controls {
        Controls { material: Material::Sand, brush: brush.clamp(1, MAX_BRUSH), paused: false, quit: false }
    }

    /// Applies an action. `Save` and `Load` need file IO, so they are returned for the caller to handle; everything else returns `None`.
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

/// Paints a disc of `radius` cells centred on `(cx, cy)` with `mat`, only into Empty cells
/// (so painting never deletes anything). Cells outside the grid are skipped.
pub fn paint(world: &mut World, cx: i32, cy: i32, mat: Material, radius: u32) {
    let r = radius as i32;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy > r * r {
                continue;
            }
            if let Some(c) = world.cells.get_mut(cx + dx, cy + dy) {
                if c.mat != Material::Empty {
                    continue;
                }
                c.mat = mat;
                c.fuel = if mat == Material::Fire { DEFAULT_FUEL } else { 0 };
                c.temp = match mat {
                    Material::Lava => LAVA_TEMP,
                    Material::Fire => 800.0,
                    _ => 293.0,
                };
            }
        }
    }
}
