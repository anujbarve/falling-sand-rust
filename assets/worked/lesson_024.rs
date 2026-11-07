fn pick(mouse_x:f32,mouse_y:f32,view_w:f32,view_h:f32)->Option<(i32,i32)> {
    if !mouse_x.is_finite() || !mouse_y.is_finite() || mouse_x<0.0
        || mouse_y<0.0 || mouse_x>=view_w || mouse_y>=view_h {return None;}
    Some(((mouse_x/4.0).floor() as i32,(mouse_y/4.0).floor() as i32))
}
fn main() {
    assert_eq!(pick(48.5,20.1,1280.0,720.0),Some((12,5)));
    assert_eq!(pick(-0.1,20.0,1280.0,720.0),None);
    assert_eq!(pick(1280.0,20.0,1280.0,720.0),None);
    println!("view picking checks passed");
}
