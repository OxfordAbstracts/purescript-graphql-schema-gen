use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

/// Every file written during this run, canonicalized. Used by
/// `remove_stale_files` to clean up files from previous runs without
/// trashing whole directories (which would bump mtimes on unchanged files
/// and force purs to re-hash everything).
static WRITTEN: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

fn written() -> &'static Mutex<HashSet<PathBuf>> {
    WRITTEN.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn write(path: &str, contents: &str) -> () {
    let file_name = Path::new(path);
    // Leave identical files untouched so downstream tools (git, purs) see
    // fewer changes.
    let unchanged = fs::read_to_string(file_name)
        .map(|existing| existing == contents)
        .unwrap_or(false);
    if !unchanged {
        if let Some(p) = file_name.parent() {
            fs::create_dir_all(p).expect("Failed to create directory for new file.");
        };
        fs::write(file_name, contents).expect(&format!("Failed to write file at {path}"));
    }
    if let Ok(canonical) = fs::canonicalize(file_name) {
        written()
            .lock()
            .expect("Write registry lock poisoned.")
            .insert(canonical);
    }
}

/// Delete generated-looking files (.purs, spago.yaml, .gitignore) under `dir`
/// that were not written during this run, then prune directories left empty.
/// Call after all generation has finished.
pub fn remove_stale_files(dir: &str) {
    let registry = written()
        .lock()
        .expect("Write registry lock poisoned.")
        .clone();
    remove_stale_in(Path::new(dir), &registry);
}

fn is_generated_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    path.extension().map(|e| e == "purs").unwrap_or(false)
        || name == "spago.yaml"
        || name == ".gitignore"
}

fn remove_stale_in(dir: &Path, registry: &HashSet<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            remove_stale_in(&path, registry);
            // remove_dir only succeeds on empty directories
            fs::remove_dir(&path).ok();
        } else if is_generated_file(&path) {
            let canonical = fs::canonicalize(&path).unwrap_or(path.clone());
            if !registry.contains(&canonical) {
                fs::remove_file(&path).ok();
            }
        }
    }
}
