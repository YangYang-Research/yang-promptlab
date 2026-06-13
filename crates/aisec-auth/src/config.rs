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
    /// Working directory for the Node runner (`node_modules` resolution).
    #[serde(default)]
    pub runner_workdir: Option<PathBuf>,
    /// Directory containing Playwright browser bundles (`PLAYWRIGHT_BROWSERS_PATH`).
    #[serde(default)]
    pub playwright_browsers_path: Option<PathBuf>,
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
            runner_workdir: None,
            playwright_browsers_path: None,
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

    pub fn with_playwright_bundle(
        mut self,
        node_bin: impl Into<PathBuf>,
        runner: impl Into<PathBuf>,
        runner_workdir: impl Into<PathBuf>,
        browsers_path: impl Into<PathBuf>,
    ) -> Self {
        let node = node_bin.into();
        self.node_bin = node.to_string_lossy().into_owned();
        self.playwright_runner = Some(runner.into());
        self.runner_workdir = Some(runner_workdir.into());
        self.playwright_browsers_path = Some(browsers_path.into());
        self
    }
}
