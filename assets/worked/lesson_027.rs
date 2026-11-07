use std::collections::HashMap;
fn main() {
    let mut chunks:HashMap<(i32,i32),usize>=HashMap::new();
    chunks.insert((100_000,0),4096);
    chunks.insert((0,0),4096);
    assert_eq!(chunks.len(),2);
    assert_eq!(chunks.get(&(1,0)),None);
    let mut keys:Vec<_>=chunks.keys().copied().collect();
    keys.sort_unstable();
    println!("ordered keys: {keys:?}");
    assert_eq!(chunks.remove(&(100_000,0)),Some(4096));
}
