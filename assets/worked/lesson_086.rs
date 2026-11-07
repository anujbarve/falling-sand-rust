#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell { material: u8, moisture: u8 }
#[derive(Clone, Copy)]
struct Edit { index: usize, before: Cell, after: Cell }
fn undo(world: &mut [Cell], edits: &[Edit]) -> (usize, usize) {
    let mut restored = 0;
    let mut skipped = 0;
    for edit in edits.iter().rev() {
        match world.get_mut(edit.index) {
            Some(live) if *live == edit.after => {
                *live = edit.before;
                restored += 1;
            }
            _ => skipped += 1,
        }
    }
    (restored, skipped)
}
fn main() {
    let air = Cell { material: 0, moisture: 0 };
    let wood = Cell { material: 1, moisture: 0 };
    let wet_wood = Cell { material: 1, moisture: 50 };
    let mut world = [wood, wet_wood];
    let edits = [Edit { index: 0, before: air, after: wood },
                 Edit { index: 1, before: air, after: wood }];
    assert_eq!(undo(&mut world, &edits), (1, 1));
    assert_eq!(world, [air, wet_wood]);
    println!("restored 1; skipped 1 changed cell");
}
