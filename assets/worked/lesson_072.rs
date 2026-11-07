use std::sync::mpsc::sync_channel;
use std::thread;
#[derive(Debug)]
struct ChunkReply { epoch: u64, x: i32, cells: Vec<u8> }
fn main() {
    let (tx, rx) = sync_channel(2);
    let worker = thread::spawn(move || {
        let reply = ChunkReply { epoch: 7, x: -1, cells: vec![0; 4096] };
        tx.send(reply).expect("demo receiver exists");
    });
    let live_epoch = 7;
    let reply = rx.recv().expect("demo worker sends one reply");
    if reply.epoch == live_epoch {
        println!("install chunk {} with {} cells", reply.x, reply.cells.len());
    }
    worker.join().expect("demo worker completes");
}
