#[derive(Default)]
struct Evidence { fire_spread: bool, rain_stopped: bool,
    wet_wood_resisted: bool, roof_fell: bool, tick_ms: f32 }
fn gate(e: &Evidence) -> bool {
    e.fire_spread && e.rain_stopped && e.wet_wood_resisted &&
    e.roof_fell && e.tick_ms <= 10.0
}
#[test]
fn every_required_result_matters() {
    let mut e=Evidence { fire_spread:true, rain_stopped:true,
        wet_wood_resisted:true, roof_fell:false, tick_ms:8.0 };
    assert!(!gate(&e));
    e.roof_fell=true;
    assert!(gate(&e));
}
