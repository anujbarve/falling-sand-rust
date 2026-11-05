use sandbox_core::grid::Grid;

#[test]
fn dimensions() {
    let g = Grid::new(4, 3, 0u8);
    assert_eq!(g.width(), 4);
    assert_eq!(g.height(), 3);
}

#[test]
fn set_and_get() {
    let mut g = Grid::new(4, 3, 0u8);
    assert!(g.set(3, 2, 9));
    assert_eq!(g.get(3, 2), Some(&9));
    assert!(!g.set(4, 0, 1));
    assert_eq!(g.get(-1, 0), None);
    assert_eq!(g.get(0, 3), None);
}

#[test]
fn index_round_trips() {
    let mut g = Grid::new(4, 3, 0u8);
    for y in 0..3 {
        for x in 0..4 {
            g.set(x, y, (y * 4 + x) as u8);
        }
    }
    for y in 0..3 {
        for x in 0..4 {
            let idx = g.index(x, y).unwrap();
            assert_eq!(g.as_slice()[idx], (y * 4 + x) as u8);
        }
    }
    *g.get_mut(1, 1).unwrap() = 99;
    let i = g.index(1, 1).unwrap();
    assert_eq!(g.as_mut_slice()[i], 99);
}
