#[derive(Debug, PartialEq)]
struct Moisture { absorbed: u8, film: u8 }
fn rain_hit(state: &mut Moisture, capacity: u8) -> bool {
    state.film = 100;
    if state.absorbed < capacity {
        state.absorbed += 1;
        true
    } else { false }
}
#[test]
fn metal_gets_wet_without_absorbing() {
    let mut metal = Moisture { absorbed: 0, film: 0 };
    assert!(!rain_hit(&mut metal, 0));
    assert_eq!(metal, Moisture { absorbed: 0, film: 100 });
    let mut soil = Moisture { absorbed: 0, film: 0 };
    assert!(rain_hit(&mut soil, 80));
    assert_eq!(soil.absorbed, 1);
}
