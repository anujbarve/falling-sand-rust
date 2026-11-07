use std::collections::{BTreeSet, VecDeque};
#[derive(Default)]
struct Pending { order: VecDeque<u32>, keys: BTreeSet<u32> }
impl Pending {
    fn request(&mut self, id: u32) {
        if self.keys.insert(id) { self.order.push_back(id); }
    }
    fn batch(&mut self, limit: usize) -> Vec<u32> {
        let mut out=Vec::new();
        while out.len()<limit {
            let Some(id)=self.order.pop_front() else { break; };
            self.keys.remove(&id);
            out.push(id);
        }
        out
    }
}
fn main() {
    let mut q=Pending::default();
    for _ in 0..1000 { q.request(7); }
    q.request(8); q.request(9);
    assert_eq!(q.order.len(),3);
    assert_eq!(q.batch(2),vec![7,8]);
    assert_eq!(q.batch(2),vec![9]);
    println!("1002 requests became 3 unique pending jobs");
}
