#[derive(Clone,Copy)]
struct Meta { key:(i32,i32),active:bool,changed:bool,last_active:u64 }
fn victim(chunks:&[Meta])->Option<(i32,i32)> {
    chunks.iter().filter(|c|!c.active && !c.changed)
        .min_by_key(|c|(c.last_active,c.key)).map(|c|c.key)
}
fn main() {
    let chunks=[
        Meta{key:(0,0),active:false,changed:true,last_active:1},
        Meta{key:(1,0),active:false,changed:false,last_active:2},
        Meta{key:(2,0),active:true,changed:false,last_active:0},
    ];
    assert_eq!(victim(&chunks),Some((1,0)));
    println!("edited and active chunks retained");
}
