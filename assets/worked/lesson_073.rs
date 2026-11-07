use std::{fs, io};
fn read_seed(path: &str) -> io::Result<u64> {
    let text = fs::read_to_string(path)?;
    text.trim().parse().map_err(|error| {
        io::Error::new(io::ErrorKind::InvalidData, format!("invalid seed: {error}"))
    })
}
fn main() {
    let path = "lesson-seed.txt";
    match read_seed(path) {
        Ok(seed) => println!("seed = {seed}"),
        Err(error) => eprintln!("Could not read {path}: {error}"),
    }
}
