#[derive(Debug, PartialEq)]
enum Hit { Rain, Snow }
fn precipitation(surface_temperature: i16, sky_exposed: bool,
                 active: bool) -> Option<Hit> {
    if !sky_exposed || !active { return None; }
    Some(if surface_temperature < 0 { Hit::Snow } else { Hit::Rain })
}
#[test]
fn surface_climate_and_shelter_decide() {
    assert_eq!(precipitation(-2,true,true), Some(Hit::Snow));
    assert_eq!(precipitation(2,true,true), Some(Hit::Rain));
    assert_eq!(precipitation(2,false,true), None);
    assert_eq!(precipitation(2,true,false), None);
}
