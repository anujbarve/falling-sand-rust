//! Tests for the dirty-chunk map.

use sandbox_core::chunk::Chunks;

#[test]
fn marking_a_cell_activates_only_its_chunk() {
    let mut c = Chunks::new(64, 48);
    assert_eq!((c.size, c.w, c.h), (16, 4, 3));
    c.mark(17, 3);
    for cy in 0..c.h {
        for cx in 0..c.w {
            assert_eq!(c.is_active(cx, cy), (cx, cy) == (1, 0), "chunk ({cx},{cy})");
        }
    }
}

#[test]
fn clear_deactivates_everything() {
    let mut c = Chunks::new(64, 48);
    c.mark(0, 0);
    c.mark(63, 47);
    c.clear();
    assert!(c.active.iter().all(|a| !a));
}

#[test]
fn out_of_range_marks_are_ignored_and_partial_chunks_count() {
    let mut c = Chunks::new(20, 20);
    assert_eq!((c.w, c.h), (2, 2));
    c.mark(-1, 5);
    c.mark(32, 5);
    c.mark(5, 32);
    assert!(c.active.iter().all(|a| !a));
    c.mark(19, 19);
    assert!(c.is_active(1, 1));
    assert!(!c.is_active(5, 5), "out-of-range chunk is inactive");
}

#[test]
fn spread_from_activates_the_eight_neighbours() {
    let mut src = Chunks::new(64, 64);
    src.mark(40, 40); // chunk (2, 2)
    let mut dst = Chunks::new(64, 64);
    dst.spread_from(&src);
    for cy in 0..4 {
        for cx in 0..4 {
            let near = (1..=3).contains(&cx) && (1..=3).contains(&cy);
            assert_eq!(dst.is_active(cx, cy), near, "chunk ({cx},{cy})");
        }
    }
}
