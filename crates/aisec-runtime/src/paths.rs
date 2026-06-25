use std::path::{Path, PathBuf};

/// Model vault directory under the app data root.
pub fn models_dir(data_root: impl AsRef<Path>) -> PathBuf {
    data_root.as_ref().join("models")
}

/// Runtime metadata directory.
pub fn runtime_dir(data_root: impl AsRef<Path>) -> PathBuf {
    data_root.as_ref().join("runtime")
}

pub fn same_paths(a: &Path, b: &Path) -> bool {
    std::fs::canonicalize(a)
        .ok()
        .zip(std::fs::canonicalize(b).ok())
        .map(|(a, b)| a == b)
        .unwrap_or_else(|| a == b)
}
