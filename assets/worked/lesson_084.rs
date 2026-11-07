fn visible_rain(x:i32,y:i32,surface:&[i32])->bool {
    x>=0 && (x as usize)<surface.len() && y>=0 && y<surface[x as usize]
}
fn main() {
    // Columns 2 and 3 have a roof at row 4; others have ground at row 10.
    let surface=[10,10,4,4,10,10];
    assert!(visible_rain(2,3,&surface));
    assert!(!visible_rain(2,4,&surface));
    assert!(!visible_rain(2,8,&surface));
    assert!(visible_rain(4,8,&surface));
    assert!(!visible_rain(-1,0,&surface));
    for (x,y) in [(4,2),(3,4),(2,6),(1,8)] {
        println!("pixel ({x},{y}) visible={}",visible_rain(x,y,&surface));
    }
}
