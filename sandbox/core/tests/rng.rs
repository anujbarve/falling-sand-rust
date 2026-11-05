use sandbox_core::rng::Rng;

#[test]
fn same_seed_same_sequence() {
    let mut a = Rng::new(7);
    let mut b = Rng::new(7);
    for _ in 0..1000 {
        assert_eq!(a.next_u32(), b.next_u32());
    }
}

#[test]
fn zero_seed_is_not_degenerate() {
    let mut r = Rng::new(0);
    assert!((0..10).any(|_| r.next_u32() != 0));
}

#[test]
fn f32_in_unit_range() {
    let mut r = Rng::new(3);
    for _ in 0..10_000 {
        let v = r.next_f32();
        assert!((0.0..1.0).contains(&v));
    }
}

#[test]
fn range_and_chance() {
    let mut r = Rng::new(5);
    for _ in 0..1000 {
        let v = r.range_f32(-2.0, 3.0);
        assert!((-2.0..3.0).contains(&v));
        assert!(!r.chance(0.0));
        assert!(r.chance(1.0));
    }
}
