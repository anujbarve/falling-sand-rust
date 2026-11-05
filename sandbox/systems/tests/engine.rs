use sandbox_core::rng::Rng;
use sandbox_core::world::World;
use sandbox_core::time::DT;
use sandbox_systems::{run_tick, registry, System};
use std::cell::RefCell;
use std::rc::Rc;

struct Counter(Rc<RefCell<u32>>);
impl System for Counter {
    fn name(&self) -> &'static str { "counter" }
    fn step(&mut self, _w: &mut World, _r: &mut Rng, _dt: f32) { *self.0.borrow_mut() += 1; }
}

struct Recorder(&'static str, Rc<RefCell<Vec<&'static str>>>);
impl System for Recorder {
    fn name(&self) -> &'static str { self.0 }
    fn step(&mut self, _w: &mut World, _r: &mut Rng, _dt: f32) { self.1.borrow_mut().push(self.0); }
}

#[test]
fn counting_system_runs_once_per_tick() {
    let n = Rc::new(RefCell::new(0));
    let mut systems: Vec<Box<dyn System>> = vec![Box::new(Counter(n.clone()))];
    let mut w = World::new(4, 4);
    let mut rng = Rng::new(1);
    run_tick(&mut w, &mut systems, &mut rng);
    assert_eq!(*n.borrow(), 1);
    run_tick(&mut w, &mut systems, &mut rng);
    assert_eq!(*n.borrow(), 2);
    assert!((w.time - 2.0 * DT).abs() < 1e-6);
}

#[test]
fn systems_run_in_registered_order() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut systems: Vec<Box<dyn System>> =
        vec![Box::new(Recorder("a", log.clone())), Box::new(Recorder("b", log.clone()))];
    let mut w = World::new(4, 4);
    run_tick(&mut w, &mut systems, &mut Rng::new(1));
    assert_eq!(*log.borrow(), vec!["a", "b"]);
}

#[test]
fn run_tick_clears_updated_flags_and_keeps_events() {
    let mut w = World::new(4, 4);
    for c in w.cells.as_mut_slice() { c.updated = true; }
    w.events.push(sandbox_core::event::Event::Sound { kind: sandbox_core::event::SoundKind::Splash, intensity: 1.0, x: 0.0, y: 0.0 });
    run_tick(&mut w, &mut [], &mut Rng::new(1));
    assert!(w.cells.as_slice().iter().all(|c| !c.updated));
    assert_eq!(w.events.len(), 1);
}

#[test]
fn registry_has_fixed_names_in_order() {
    let names: Vec<_> = registry().iter().map(|s| s.name()).collect();
    assert_eq!(names, vec![
        "granular", "liquid", "air", "thermal", "humidity", "fire", "volcano",
        "particles", "collisions", "explosions", "chemistry", "ocean", "lighting",
    ]);
}
