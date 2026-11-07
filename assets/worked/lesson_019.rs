fn index(x:i32,y:i32,w:i32,h:i32)->Option<usize> {
    if x<0 || y<0 || x>=w || y>=h { None }
    else { Some(y as usize*w as usize+x as usize) }
}
fn paint(cells:&mut [u8],x:i32,y:i32,w:i32,h:i32,value:u8)->bool {
    match index(x,y,w,h) {
        Some(i) => { cells[i]=value; true }
        None => false,
    }
}
fn main() {
    let mut cells=vec![0;8];
    assert!(paint(&mut cells,2,1,4,2,7));
    assert!(!paint(&mut cells,-1,0,4,2,9));
    assert_eq!(cells[0],0);
    assert_eq!(cells[6],7);
    println!("edge paint is a no-op");
}
