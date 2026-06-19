use std::path::{Path, PathBuf};

use crate::paths::{bundled_llama_server_binary, models_dir};
use crate::runtime::{default_llama_host, default_llama_port};

/// Configuration for the embedded llama.cpp runtime supervisor.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Path to the `llama-server` / `llama-server.exe` binary.
    pub binary: PathBuf,
    /// Directory for GGUF model vault files.
    pub models_dir: PathBuf,
    /// llama-server HTTP API base URL (default `http://127.0.0.1:8081`).
    pub base_url: String,
    pub host: String,
    pub port: u16,
}

impl RuntimeConfig {
    pub fn new(app_root: impl AsRef<Path>, data_root: impl AsRef<Path>) -> Self {
        let host = default_llama_host();
        let port = default_llama_port();
        let base_url = default_llama_base_url();
        Self {
            binary: bundled_llama_server_binary(app_root),
            models_dir: models_dir(data_root),
            base_url,
            host,
            port,
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

/// Default llama-server HTTP base URL.
pub fn default_llama_base_url() -> String {
    std::env::var("AISEC_LLAMA_BASE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(|host| {
            if host.starts_with("http://") || host.starts_with("https://") {
                host
            } else {
                format!("http://{host}")
            }
        })
        .unwrap_or_else(|| {
            format!(
                "http://{}:{}",
                default_llama_host(),
                default_llama_port()
            )
        })
}

/// Deprecated alias kept for callers that still reference the Ollama default URL.
#[deprecated(note = "use default_llama_base_url(); Ollama is no longer the embedded runtime")]
pub fn default_ollama_base_url() -> String {
    default_llama_base_url()
}
