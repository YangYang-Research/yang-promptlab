use std::path::{Path, PathBuf};

pub fn models_dir(data_root: impl AsRef<Path>) -> PathBuf {
    data_root.as_ref().join("models")
}

pub fn bundled_runtime_dir(app_root: impl AsRef<Path>) -> PathBuf {
    app_root.as_ref().join("runtime")
}

pub fn bundled_ollama_binary(app_root: impl AsRef<Path>) -> PathBuf {
    let base = bundled_runtime_dir(app_root);
    if cfg!(target_os = "windows") {
        base.join("ollama.exe")
    } else {
        base.join("ollama")
    }
}
