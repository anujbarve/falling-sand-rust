fn main() {
    let row = 158;
    let floor_row = 160;
    if row < floor_row - 1 {
        println!("moving");
    } else {
        println!("resting or invalid starting state");
    }
}
