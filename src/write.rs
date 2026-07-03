use std::{fs, path::Path};

pub fn write(path: &str, contents: &str) -> () {
    let file_name = Path::new(path);
    // Leave identical files untouched so downstream tools (git, purs) see
    // fewer changes.
    if let Ok(existing) = fs::read_to_string(file_name) {
        if existing == contents {
            return;
        }
    }
    if let Some(p) = file_name.parent() {
        fs::create_dir_all(p).expect("Failed to create directory for new file.");
    };
    fs::write(file_name, contents).expect(&format!("Failed to write file at {path}"));
}
