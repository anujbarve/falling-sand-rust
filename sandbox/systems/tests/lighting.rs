//! Behaviour tests for the `Lighting` system.

use sandbox_core::material::Material;
use sandbox_core::rng::Rng;
use sandbox_core::world::World;
use sandbox_systems::lighting::Lighting;
use sandbox_systems::System;

fn light_world(w: &mut World) {
    let mut rng = Rng::new(1);
    Lighting.step(w, &mut rng, 1.0 / 60.0);
}

fn set(w: &mut World, x: i32, y: i32, m: Material) {
    w.cells.get_mut(x, y).unwrap().mat = m;
}

fn at(w: &World, x: usize, y: usize) -> f32 {
    w.light[y * w.cells.width() + x]
}

#[test]
fn emitter_is_full_bright_and_falls_off_monotonically() {
    let mut w = World::new(40, 20);
    set(&mut w, 10, 10, Material::Lava);
    light_world(&mut w);
    assert_eq!(at(&w, 10, 10), 1.0);
    let mut prev = 1.0;
    for d in 1..=12 {
        let v = at(&w, 10 + d, 10);
        assert!(v < prev, "d={d}: {v} !< {prev}");
        assert!(v > 0.0);
        prev = v;
    }
    assert!(at(&w, 14, 10) < at(&w, 11, 10));
    assert_eq!(at(&w, 10 + 13, 10), 0.0, "beyond radius 12 stays dark");
}

#[test]
fn fire_and_ember_emit_too() {
    for m in [Material::Fire, Material::Ember] {
        let mut w = World::new(20, 20);
        set(&mut w, 5, 5, m);
        light_world(&mut w);
        assert_eq!(at(&w, 5, 5), 1.0, "{m:?}");
    }
}

#[test]
fn stone_wall_blocks_most_light() {
    let probe = |wall: Option<Material>| {
        let mut w = World::new(30, 20);
        set(&mut w, 5, 10, Material::Lava);
        if let Some(m) = wall {
            for y in 0..20 {
                set(&mut w, 7, y, m);
            }
        }
        light_world(&mut w);
        at(&w, 9, 10)
    };
    let open = probe(None);
    assert!(open > 0.0);
    for m in [Material::Stone, Material::Rock, Material::Metal, Material::Glass] {
        let shadow = probe(Some(m));
        assert!(shadow <= open * 0.2, "{m:?}: {shadow} vs {open}");
    }
}

#[test]
fn values_in_unit_range_with_many_emitters_and_no_material_change() {
    let mut w = World::new(20, 20);
    for y in 5..15 {
        for x in 5..15 {
            set(&mut w, x, y, Material::Lava);
        }
    }
    let before: Vec<_> = w.cells.as_slice().iter().map(|c| c.mat).collect();
    light_world(&mut w);
    assert!(w.light.iter().all(|v| (0.0..=1.0).contains(v)));
    let after: Vec<_> = w.cells.as_slice().iter().map(|c| c.mat).collect();
    assert_eq!(before, after);
}

#[test]
fn light_is_recomputed_each_tick() {
    let mut w = World::new(20, 20);
    set(&mut w, 5, 5, Material::Lava);
    light_world(&mut w);
    set(&mut w, 5, 5, Material::Empty);
    light_world(&mut w);
    assert!(w.light.iter().all(|v| *v == 0.0));
}
