//! Chemistry: a data-driven table of pairwise reactions between orthogonal neighbours.

use crate::engine::System;
use crate::fire::DEFAULT_FUEL;
use sandbox_core::event::Event;
use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;

/// Side effect of a reaction beyond rewriting the two cells.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReactionEmit {
    /// Push `Event::Ignite` at each cell that becomes Fire.
    Ignite,
    /// The reaction releases steam; the Steam is already one of the output cells, so no event is sent.
    Steam,
}

/// One row of the reaction table: `a` next to `b` becomes `out_a` and `out_b`.
#[derive(Clone, Copy, Debug)]
pub struct Reaction {
    /// Material of the first reactant.
    pub a: Material,
    /// Material of the second reactant.
    pub b: Material,
    /// What the `a` cell becomes.
    pub out_a: Material,
    /// What the `b` cell becomes.
    pub out_b: Material,
    /// Per-tick probability, in `0.0..=1.0`, that an adjacent pair reacts.
    pub chance: f32,
    /// Optional side effect.
    pub emit: Option<ReactionEmit>,
}

/// The reaction table. Each `(a, b)` pair appears once; matching works in both orders.
pub const REACTIONS: &[Reaction] = &[
    Reaction {
        a: Material::Acid, b: Material::Metal,
        out_a: Material::Steam, out_b: Material::Empty,
        chance: 1.0, emit: Some(ReactionEmit::Steam),
    },
    Reaction {
        a: Material::Acid, b: Material::Stone,
        out_a: Material::Empty, out_b: Material::Empty,
        chance: 0.02, emit: None,
    },
    Reaction {
        a: Material::Fire, b: Material::Gunpowder,
        out_a: Material::Fire, out_b: Material::Fire,
        chance: 0.5, emit: Some(ReactionEmit::Ignite),
    },
];

const ORTHO: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

/// Applies `REACTIONS` once per tick; a cell reacts at most once per tick.
pub struct Chemistry;

/// Finds the table row for the ordered pair, returning outputs in `(first, second)` order.
fn lookup(first: Material, second: Material) -> Option<(&'static Reaction, Material, Material)> {
    REACTIONS.iter().find_map(|r| {
        if r.a == first && r.b == second {
            Some((r, r.out_a, r.out_b))
        } else if r.a == second && r.b == first {
            Some((r, r.out_b, r.out_a))
        } else {
            None
        }
    })
}

fn rewrite(w: &mut World, x: i32, y: i32, to: Material, emit: Option<ReactionEmit>) {
    let Some(c) = w.cells.get_mut(x, y) else { return };
    let changed = c.mat != to;
    c.mat = to;
    c.updated = true;
    if to == Material::Fire && c.fuel == 0 {
        c.fuel = DEFAULT_FUEL;
    }
    if changed && to == Material::Fire && emit == Some(ReactionEmit::Ignite) {
        w.events.push(Event::Ignite { x, y });
    }
}

impl System for Chemistry {
    fn name(&self) -> &'static str {
        "chemistry"
    }

    fn step(&mut self, w: &mut World, rng: &mut Rng, _dt: f32) {
        let (width, height) = (w.cells.width() as i32, w.cells.height() as i32);
        for y in 0..height {
            for x in 0..width {
                let Some(c) = w.cells.get(x, y) else { continue };
                if c.updated {
                    continue;
                }
                let m = c.mat;
                for (dx, dy) in ORTHO {
                    let (nx, ny) = (x + dx, y + dy);
                    let Some(n) = w.cells.get(nx, ny) else { continue };
                    if n.updated {
                        continue;
                    }
                    let Some((r, out_here, out_there)) = lookup(m, n.mat) else { continue };
                    if rng.chance(r.chance) {
                        rewrite(w, x, y, out_here, r.emit);
                        rewrite(w, nx, ny, out_there, r.emit);
                        break;
                    }
                }
            }
        }
    }
}
