#[derive(Debug)]
struct Resident { revision:u64, saved_revision:u64, active:bool }
impl Resident {
    fn dirty(&self)->bool {self.revision!=self.saved_revision}
    fn acknowledge(&mut self,written:u64) {
        if written==self.revision {self.saved_revision=written;}
    }
    fn may_evict(&self)->bool {!self.active && !self.dirty()}
}
fn main() {
    let mut c=Resident {revision:4,saved_revision:3,active:false};
    let queued=c.revision;
    c.revision=5;
    c.acknowledge(queued);
    assert!(!c.may_evict());
    c.acknowledge(5);
    assert!(c.may_evict());
    println!("stale acknowledgement cannot discard revision 5");
}
