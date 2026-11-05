use sandbox_core::material::Material;
use sandbox_core::save::{load_world, save_world, SaveError};
use sandbox_core::vec2::Vec2;
use sandbox_core::world::World;
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sbx_save_{}_{}", std::process::id(), name));
    let _ = std::fs::remove_file(&d);
    d
}

fn sample() -> World {
    let mut w = World::new(7, 5);
    let mats = [Material::Sand, Material::Water, Material::Ember, Material::Steam];
    for (i, c) in w.cells.as_mut_slice().iter_mut().enumerate() {
        c.mat = mats[i % 4];
        c.temp = 293.0 + i as f32 * 0.37;
        c.fuel = (i * 7 % 256) as u8;
    }
    for i in 0..35 {
        w.air.vel[i] = Vec2 { x: i as f32 * 0.1, y: -(i as f32) / 3.0 };
        w.air.pressure[i] = i as f32 * 0.01;
        w.air.vapor[i] = (i % 10) as f32 / 10.0;
    }
    w
}

#[test]
fn round_trip_preserves_cells_and_air() {
    let w = sample();
    let p = tmp("rt.sav");
    save_world(&w, &p).unwrap();
    let r = load_world(&p).unwrap();
    assert_eq!(r.cells.width(), 7);
    assert_eq!(r.cells.height(), 5);
    for (a, b) in w.cells.as_slice().iter().zip(r.cells.as_slice()) {
        assert_eq!(a.mat, b.mat);
        assert_eq!(a.temp, b.temp);
        assert_eq!(a.fuel, b.fuel);
    }
    assert_eq!(w.air.vel, r.air.vel);
    assert_eq!(w.air.pressure, r.air.pressure);
    assert_eq!(w.air.vapor, r.air.vapor);
}

#[test]
fn unknown_version_is_err() {
    let p = tmp("v99.sav");
    std::fs::write(&p, "version 99\nsize 1 1\n").unwrap();
    assert!(matches!(load_world(&p), Err(SaveError::BadVersion(99))));
}

#[test]
fn truncated_file_is_corrupt_not_panic() {
    let p = tmp("trunc.sav");
    save_world(&sample(), &p).unwrap();
    let s = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, &s[..s.len() / 2]).unwrap();
    assert!(matches!(load_world(&p), Err(SaveError::Corrupt(_))));
}

#[test]
fn garbage_and_empty_are_err() {
    let p = tmp("junk.sav");
    std::fs::write(&p, "").unwrap();
    assert!(load_world(&p).is_err());
    std::fs::write(&p, "version 1\nsize 2 1\n99 x y\n").unwrap();
    assert!(matches!(load_world(&p), Err(SaveError::Corrupt(_))));
    std::fs::write(&p, "version 1\nsize 99999999 99999999\n").unwrap();
    assert!(matches!(load_world(&p), Err(SaveError::Corrupt(_))));
    assert!(matches!(load_world(&tmp("none.sav")), Err(SaveError::Io(_))));
}

#[test]
fn failed_save_leaves_existing_file_unchanged() {
    let p = tmp("atomic.sav");
    save_world(&sample(), &p).unwrap();
    let before = std::fs::read(&p).unwrap();
    // Block the temp path with a directory so the temp write fails.
    let mut t = p.clone().into_os_string();
    t.push(".tmp");
    std::fs::create_dir(&t).unwrap();
    assert!(save_world(&World::new(2, 2), &p).is_err());
    assert_eq!(std::fs::read(&p).unwrap(), before);
    std::fs::remove_dir(&t).unwrap();
}
