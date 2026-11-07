#[derive(Debug, PartialEq)]
struct Gains { wind: f32, rain: f32, fire: f32 }
fn gains(wind: f32, rain: f32, burning: usize,
         muted: bool, event_duck: f32) -> Gains {
    let master = if muted { 0.0 } else { 1.0 };
    let bed = master * event_duck.clamp(0.0, 1.0);
    Gains {
        wind: wind.clamp(0.0, 1.0) * 0.15 * bed,
        rain: rain.clamp(0.0, 1.0) * 0.35 * bed,
        fire: (burning as f32 / 200.0).clamp(0.0, 1.0) * 0.30 * bed,
    }
}
fn main() {
    let quiet = gains(0.4, 0.0, 0, false, 1.0);
    let fire = gains(0.4, 0.0, 5000, false, 1.0);
    assert!(quiet.fire == 0.0 && fire.fire <= 0.30);
    assert_eq!(gains(1.0, 1.0, 200, true, 1.0),
               Gains { wind: 0.0, rain: 0.0, fire: 0.0 });
    println!("bounded fire; immediate mute; independent world state");
}
