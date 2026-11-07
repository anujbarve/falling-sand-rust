#[derive(Clone, Copy)]
enum Generator { V1, V2 }
fn parse_version(id: u16) -> Result<Generator,&'static str> {
    match id {1=>Ok(Generator::V1),2=>Ok(Generator::V2),_=>Err("unsupported generator")}
}
fn height(version: Generator,x:i32) -> i32 {
    match version {
        Generator::V1 => 20+x.rem_euclid(4),
        Generator::V2 => 40+x.rem_euclid(8),
    }
}
fn main() {
    let world_generator=parse_version(1).unwrap();
    let new_default=Generator::V2;
    assert_eq!(height(world_generator,64),20);
    assert_eq!(height(new_default,64),40);
    assert!(parse_version(99).is_err());
    println!("old world keeps its generator; new worlds may use v2");
}
