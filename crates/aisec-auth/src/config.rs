use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Authentication engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthEngineConfig {
    /// Directory for Playwright storageState JSON files.
    pub vault_dir: PathBuf,
    /// Path to `runner.mjs` (auto-detected if None).
    pub playwright_runner: Option<PathBuf>,
    /// Node.js executable.
    pub node_bin: String,
    pub default_timeout: Duration,
    pub headless: bool,
}

impl Default for AuthEngineConfig {
    fn default() -> Self {
        Self {
            vault_dir: PathBuf::from("./data/auth-vault"),
            playwright_runner: None,
            node_bin: "node".into(),
            default_timeout: Duration::from_secs(30),
            headless: true,
        }
    }
}

impl AuthEngineConfig {
    pub fn with_vault_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.vault_dir = dir.into();
        self
    }
}
