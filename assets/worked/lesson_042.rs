trait Choice { fn choose_left(&self) -> bool; }
struct Fixed(bool);
impl Choice for Fixed {
    fn choose_left(&self) -> bool { self.0 }
}
struct TickParity(u64);
impl Choice for TickParity {
    fn choose_left(&self) -> bool { self.0 & 1 == 0 }
}
fn direction(source: &impl Choice) -> i32 {
    if source.choose_left() { -1 } else { 1 }
}
#[test]
fn the_rule_accepts_scripted_and_tick_sources() {
    assert_eq!(direction(&Fixed(true)), -1);
    assert_eq!(direction(&Fixed(false)), 1);
    assert_eq!(direction(&TickParity(2)), -1);
    assert_eq!(direction(&TickParity(3)), 1);
}
