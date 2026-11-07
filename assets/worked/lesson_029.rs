#[derive(Clone,Copy,Debug,PartialEq,Eq)]
struct Rect { left:i32,top:i32,right:i32,bottom:i32 }
fn intersect(a:Rect,b:Rect)->Option<Rect> {
    let r=Rect{left:a.left.max(b.left),top:a.top.max(b.top),
        right:a.right.min(b.right),bottom:a.bottom.min(b.bottom)};
    if r.left<r.right && r.top<r.bottom {Some(r)} else {None}
}
fn main() {
    let chunk=Rect{left:0,top:0,right:64,bottom:64};
    let active=Rect{left:20,top:10,right:80,bottom:40};
    let clip=intersect(chunk,active).unwrap();
    assert_eq!(clip,Rect{left:20,top:10,right:64,bottom:40});
    println!("{} active cells in this chunk",(clip.right-clip.left)*(clip.bottom-clip.top));
}
