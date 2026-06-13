use std::path::{Path, PathBuf};

use crate::paths::{bundled_ollama_binary, models_dir};

/// Configuration for the embedded Ollama runtime supervisor.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Path to the `ollama` / `ollama.exe` binary.
    pub binary: PathBuf,
    /// Directory for pulled models (`OLLAMA_MODELS`).
    pub models_dir: PathBuf,
    /// Ollama HTTP API base URL (default `http://127.0.0.1:11434`).
    pub base_url: String,
}

impl RuntimeConfig {
    pub fn new(app_root: impl AsRef<Path>, data_root: impl AsRef<Path>) -> Self {
        Self {
            binary: bundled_ollama_binary(app_root),
            models_dir: models_dir(data_root),
            base_url: default_ollama_base_url(),
        }
    }

    pub fn with_binary(mut self, binary: impl Into<PathBuf>) -> Self {
        self.binary = binary.into();
        self
    }

    pub fn binary_available(&self) -> bool {
        self.binary.is_file()
    }
}

pub fn default_ollama_base_url() -> String {
    std::env::var("OLLAMA_HOST")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(|host| {
            if host.starts_with("http://") || host.starts_with("https://") {
                host
            } else {
                format!("http://{host}")
            }
        })
        .unwrap_or_else(|| "http://127.0.0.1:11434".into())
}
