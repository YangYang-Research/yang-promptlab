use std::path::{Path, PathBuf};

use crate::paths::models_dir;

/// Configuration for the remote-oriented runtime host.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Directory under the app data root for model-related files.
    pub models_dir: PathBuf,
}

impl RuntimeConfig {
    pub fn new(data_root: impl AsRef<Path>) -> Self {
        Self {
            models_dir: models_dir(data_root),
        }
    }
}
