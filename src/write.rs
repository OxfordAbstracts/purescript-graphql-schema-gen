use std::{fs, path::Path};

pub fn write(path: &str, contents: &str) -> () {
    let file_name = Path::new(path);
    if let Some(p) = file_name.parent() {
        fs::create_dir_all(p).unwrap();
    };
    fs::write(file_name, contents).unwrap();
}
