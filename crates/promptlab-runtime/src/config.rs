use std::path::{Path, PathBuf};

use crate::local_runtime_adapter::GfxBackend;
use crate::paths::models_dir;

/// Configuration for the embedded libllama runtime.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Directory for GGUF model vault files.
    pub models_dir: PathBuf,
    /// Selected inference backend (auto-detect when Auto).
    pub backend: GfxBackend,
}

impl RuntimeConfig {
    pub fn new(data_root: impl AsRef<Path>) -> Self {
        Self {
            models_dir: models_dir(data_root),
            backend: GfxBackend::Auto,
        }
    }

    pub fn with_backend(mut self, backend: GfxBackend) -> Self {
        self.backend = backend;
        self
    }
}
