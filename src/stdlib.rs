//! Source-backed standard library. Only explicitly imported modules enter a pack.

use std::path::{Path, PathBuf};

pub(crate) const MODULES: &[&str] = &["math", "random", "state", "time"];

pub(crate) fn source(segments: &[String]) -> Option<&'static str> {
    let [root, module] = segments else {
        return None;
    };
    if root != "std" {
        return None;
    }
    match module.as_str() {
        "math" => Some(include_str!("../stdlib/math.mcl")),
        "random" => Some(include_str!("../stdlib/random.mcl")),
        "state" => Some(include_str!("../stdlib/state.mcl")),
        "time" => Some(include_str!("../stdlib/time.mcl")),
        _ => None,
    }
}

pub(crate) fn virtual_path(directory: &Path, module: &str) -> PathBuf {
    directory.join("__mcl_std").join(format!("{module}.mcl"))
}

pub(crate) fn is_virtual_path(path: &Path, directory: &Path) -> bool {
    path.strip_prefix(directory)
        .ok()
        .is_some_and(|relative| relative.starts_with("__mcl_std"))
}
