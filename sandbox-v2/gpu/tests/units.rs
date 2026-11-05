//! Physical-unit tests: tick conversion and free-fall timing. CPU only.

use sandbox_v2_gpu::reference::{Reference, SAND, WALL};
use sandbox_v2_gpu::units::{ticks, DT, GRAVITY_CELLS};

#[test]
fn half_a_second_is_thirty_ticks() {
    assert_eq!(ticks(0.5), 30);
    assert_eq!(ticks(0.0), 0);
    assert_eq!(ticks(0.1), 6);
    assert_eq!(ticks(6.0), 360);
}

#[test]
fn sand_dropped_one_metre_lands_in_about_0_45_s() {
    // Floor at y = 101, grain starts at y = 0 and rests at y = 100: 100 cells = 1 m.
    let mut r = Reference::new(8, 102);
    for x in 0..8 {
        r.set(x, 101, WALL);
    }
    r.set(4, 0, SAND);
    let mut landed = None;
    for t in 0..120u32 {
        r.tick(t, 1);
        if r.get(4, 100) == SAND {
            landed = Some(t + 1);
            break;
        }
    }
    let n = landed.expect("grain never reached the floor");
    let secs = n as f32 * DT;
    let ideal = (2.0 * 100.0 / GRAVITY_CELLS).sqrt();
    println!("free fall: {n} ticks = {secs:.4} s (ideal {ideal:.4} s)");
    assert!((secs - 0.45).abs() <= 0.03, "fell in {secs} s");
}

#[test]
fn material_table_matches_enum_and_limits() {
    use sandbox_v2_gpu::materials::{Material, MATERIALS, MATERIAL_COUNT};
    use sandbox_v2_gpu::reference::{EMPTY, WATER};
    use sandbox_v2_gpu::units::{cps_to_fp, FP_ONE, MAX_STEP};
    assert_eq!(MATERIALS.len(), MATERIAL_COUNT);
    assert_eq!(Material::Empty as u32, EMPTY);
    assert_eq!(Material::Sand as u32, SAND);
    assert_eq!(Material::Water as u32, WATER);
    assert_eq!(sandbox_v2_gpu::materials::WALL, WALL);
    assert_eq!(Material::Wood.def().burn_s, 6.0);
    for m in MATERIALS.iter() {
        assert!(
            cps_to_fp(m.terminal_cps) <= MAX_STEP as i32 * FP_ONE,
            "terminal speed too high"
        );
    }
}

#[test]
fn material_ids_match_v1() {
    use sandbox_core::material::Material as V1;
    use sandbox_v2_gpu::materials::Material as V2;
    let pairs = [
        (V2::Empty, V1::Empty),
        (V2::Sand, V1::Sand),
        (V2::Water, V1::Water),
        (V2::Oil, V1::Oil),
        (V2::Stone, V1::Stone),
        (V2::Rock, V1::Rock),
        (V2::Wood, V1::Wood),
        (V2::Ash, V1::Ash),
        (V2::Metal, V1::Metal),
        (V2::Glass, V1::Glass),
        (V2::Lava, V1::Lava),
        (V2::Acid, V1::Acid),
        (V2::Gunpowder, V1::Gunpowder),
        (V2::Ice, V1::Ice),
        (V2::Fire, V1::Fire),
        (V2::Steam, V1::Steam),
        (V2::Smoke, V1::Smoke),
        (V2::Ember, V1::Ember),
    ];
    assert_eq!(pairs.len(), sandbox_v2_gpu::materials::MATERIAL_COUNT);
    for (a, b) in pairs {
        assert_eq!(a as u32, b as u8 as u32, "{a:?} id drifted from v1");
    }
}
