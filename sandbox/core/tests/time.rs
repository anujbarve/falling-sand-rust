use sandbox_core::time::{FixedStep, DT};

#[test]
fn one_and_two_ticks() {
    assert_eq!(FixedStep::new().push_frame(1.0 / 60.0), 1);
    assert_eq!(FixedStep::new().push_frame(1.0 / 30.0), 2);
}

#[test]
fn accumulates_quarter_steps() {
    let mut f = FixedStep::new();
    let got: Vec<u32> = (0..4).map(|_| f.push_frame(DT / 4.0)).collect();
    assert_eq!(got, vec![0, 0, 0, 1]);
    assert_eq!(f.push_frame(DT * 2.0), 2);
}

#[test]
fn caps_ticks_per_frame() {
    let mut f = FixedStep::new();
    assert_eq!(f.push_frame(10.0), 8);
    assert_eq!(f.push_frame(0.0), 0);
}
