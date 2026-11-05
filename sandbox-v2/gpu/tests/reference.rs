//! CPU-only tests of the parallel row-parity reference. No GPU needed.

mod common;
use common::{scene, SEED, SIZE, TICKS};
use sandbox_v2_gpu::reference::{Reference, EMPTY, SAND, WALL, WATER};

fn run(mut r: Reference, ticks: u32) -> Reference {
    for t in 0..ticks {
        r.tick(t, SEED);
    }
    r
}

#[test]
fn sand_column_falls_to_floor() {
    let mut r = Reference::new(32, 32);
    for x in 0..32 {
        r.set(x, 31, WALL);
    }
    for y in 0..6 {
        r.set(16, y, SAND);
    }
    let r = run(r, TICKS);
    assert_eq!(r.count(SAND), 6);
    assert!(
        (0..32).any(|x| r.get(x, 30) == SAND),
        "sand reached the floor"
    );
    for y in 0..31 {
        for x in 0..32 {
            if r.get(x, y) == SAND {
                let below = r.get(x, y + 1) != EMPTY;
                let dl = x == 0 || r.get(x - 1, y + 1) != EMPTY;
                let dr = x == 31 || r.get(x + 1, y + 1) != EMPTY;
                assert!(below && dl && dr, "sand at ({x},{y}) is still free to move");
            }
        }
    }
}

#[test]
fn water_spreads_sideways() {
    let mut r = Reference::new(32, 32);
    for x in 0..32 {
        r.set(x, 31, WALL);
    }
    for y in 0..8 {
        r.set(16, y, WATER);
    }
    let r = run(r, TICKS);
    assert_eq!(r.count(WATER), 8);
    let max_height = (0..32)
        .map(|x| (0..31).filter(|&y| r.get(x, y) == WATER).count())
        .max()
        .unwrap();
    assert!(
        max_height <= 2,
        "water not levelled: tallest column holds {max_height} cells"
    );
}

#[test]
fn mass_conserved_in_scene() {
    let r0 = scene();
    let (sand, water) = (r0.count(SAND), r0.count(WATER));
    let r = run(r0, TICKS);
    assert_eq!(r.count(SAND), sand);
    assert_eq!(r.count(WATER), water);
    assert_eq!(r.cells().len(), (SIZE * SIZE) as usize);
}

#[test]
fn reference_is_deterministic() {
    assert_eq!(run(scene(), TICKS).cells(), run(scene(), TICKS).cells());
}
