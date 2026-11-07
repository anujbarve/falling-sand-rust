type HeightFn=fn(i32)->i32;
fn height_v1(x:i32)->i32 { 20+x.rem_euclid(4) }
fn height_v2(x:i32)->i32 { 30+x.rem_euclid(7) }
fn generator(version:u16)->Result<HeightFn,String> {
    match version {
        1=>Ok(height_v1),
        2=>Ok(height_v2),
        _=>Err(format!("unsupported generator version {version}")),
    }
}
fn validate_format(version:u16)->Result<(),String> {
    if version==1 { Ok(()) }
    else { Err(format!("unsupported save format {version}")) }
}
fn main() {
    validate_format(1).unwrap();
    let old=generator(1).unwrap();
    let latest=generator(2).unwrap();
    assert_eq!(old(5),21);
    assert_eq!(latest(5),35);
    assert!(generator(99).is_err());
    println!("format 1 remains readable; generator 1 still returns 21");
}
