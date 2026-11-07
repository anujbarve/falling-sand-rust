fn sky_exposed(x:usize,y:usize,surface:&[usize])->bool {surface[x]==y}
fn air_exposed(x:usize,y:usize,w:usize,h:usize,solid:&[bool])->bool {
    let neighbours=[(x as isize-1,y as isize),(x as isize+1,y as isize),
        (x as isize,y as isize-1),(x as isize,y as isize+1)];
    neighbours.into_iter().any(|(nx,ny)| nx>=0 && ny>=0
        && nx<w as isize && ny<h as isize && !solid[ny as usize*w+nx as usize])
}
fn main() {
    let (w,h)=(5,5);
    let mut solid=vec![false;w*h];
    solid[1*w+2]=true; // roof above this column
    solid[3*w+2]=true; // a wall cell below, beside local air
    let mut surface=vec![h;w];
    for x in 0..w {for y in 0..h {if solid[y*w+x] {surface[x]=y;break;}}}
    assert!(!sky_exposed(2,3,&surface));
    assert!(air_exposed(2,3,w,h,&solid));
    println!("wall: sheltered from sky, touching air");
}
