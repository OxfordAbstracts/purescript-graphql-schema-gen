use std::{fs, path::Path};

pub fn write(path: &str, contents: &str) -> () {
    let file_name = Path::new(path);
    if let Some(p) = file_name.parent() {
        fs::create_dir_all(p).expect("Failed to create directory for new file.");
    };
    fs::write(file_name, contents).expect(&format!("Failed to write file."));
}
