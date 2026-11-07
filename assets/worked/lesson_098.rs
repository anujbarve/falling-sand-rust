use std::path::{Path,PathBuf};
fn asset_root(executable:&Path,mac_bundle:bool)->Result<PathBuf,String> {
    let parent=executable.parent().ok_or("executable has no parent")?;
    if mac_bundle {
        let contents=parent.parent().ok_or("missing Contents directory")?;
        Ok(contents.join("Resources").join("assets"))
    } else { Ok(parent.join("assets")) }
}
fn main() {
    let flat=asset_root(Path::new("dist/rusty-linux/game"),false).unwrap();
    let app=asset_root(Path::new("dist/Rusty.app/Contents/MacOS/game"),true).unwrap();
    assert_eq!(flat,PathBuf::from("dist/rusty-linux/assets"));
    assert_eq!(app,PathBuf::from("dist/Rusty.app/Contents/Resources/assets"));
    println!("flat: {}",flat.display());
    println!("app: {}",app.display());
}
