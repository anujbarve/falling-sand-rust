use std::{fmt, fs, io, path::{Path, PathBuf}};
#[derive(Debug)]
enum ConfigError {
    Read { path: PathBuf, source: io::Error },
    Invalid { path: PathBuf, message: String },
}
impl fmt::Display for ConfigError {
    fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result {
        match self {
            Self::Read{path,source}=>write!(f,"read {}: {}",path.display(),source),
            Self::Invalid{path,message}=>write!(f,"decode {}: {}",path.display(),message),
        }
    }
}
impl std::error::Error for ConfigError {
    fn source(&self)->Option<&(dyn std::error::Error+'static)> {
        match self { Self::Read{source,..}=>Some(source), _=>None }
    }
}
fn parse_budget(text:&str,path:&Path)->Result<u32,ConfigError> {
    text.trim().parse::<u32>().ok().filter(|n|*n>0)
        .ok_or_else(||ConfigError::Invalid{path:path.to_owned(),
            message:"expected a positive active-cell budget".to_owned()})
}
fn load_budget(path:&Path)->Result<u32,ConfigError> {
    let text=fs::read_to_string(path)
        .map_err(|source|ConfigError::Read{path:path.to_owned(),source})?;
    parse_budget(&text,path)
}
fn main() {
    let path=Path::new("settings.txt");
    assert_eq!(parse_budget("74624",path).unwrap(),74_624);
    let error=parse_budget("zero",path).unwrap_err();
    println!("{error}");
    let _loader:fn(&Path)->Result<u32,ConfigError>=load_budget;
}
