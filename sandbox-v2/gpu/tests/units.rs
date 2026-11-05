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
    assert_eq!(Material::Wall as u32, WALL);
    assert_eq!(Material::Wood.def().burn_s, 6.0);
    for m in MATERIALS.iter() {
        assert!(cps_to_fp(m.terminal_cps) <= MAX_STEP as i32 * FP_ONE, "terminal speed too high");
    }
}
