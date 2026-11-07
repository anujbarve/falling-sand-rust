#[derive(Clone, Debug, PartialEq)]
struct Particle { x: i32, velocity_x: i32, material: u8 }
#[derive(Clone, Debug, PartialEq)]
struct Snapshot { step: u64, cells: Vec<u8>, particles: Vec<Particle> }
fn main() {
    let mut live=Snapshot { step: 10, cells: vec![0,2,0],
        particles: vec![Particle{x:4,velocity_x:3,material:2}] };
    let captured=live.clone();
    live.step+=1;
    live.cells.swap(1,2);
    live.particles[0].x+=live.particles[0].velocity_x;
    assert_eq!(captured.step,10);
    assert_eq!(captured.cells,vec![0,2,0]);
    assert_eq!(captured.particles[0].x,4);
    assert_eq!(live.particles[0].x,7);
    println!("snapshot retained tick 10; live world advanced");
}
