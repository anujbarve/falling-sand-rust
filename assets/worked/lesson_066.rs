fn ambient(base:i16, weather:i16, altitude:i32, sea_level:i32)->i32 {
    i32::from(base)+i32::from(weather)-(altitude-sea_level).div_euclid(100)
}
#[test]
fn climate_uses_world_altitude_not_camera_height() {
    assert_eq!(ambient(20,-2,0,0),18);
    assert_eq!(ambient(20,-2,200,0),16);
    assert_eq!(ambient(20,-2,-100,0),19);
}
