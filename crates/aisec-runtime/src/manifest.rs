//! Persisted runtime installation manifest.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::error::{RuntimeError, RuntimeResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBackend {
    Cpu,
    Cuda,
    Metal,
    Vulkan,
}

impl RuntimeBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
            Self::Metal => "metal",
            Self::Vulkan => "vulkan",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeManifest {
    pub runtime_version: String,
    pub backend: RuntimeBackend,
    pub platform: String,
    pub install_path: PathBuf,
    pub installed: bool,
    pub verified: bool,
    pub sha256: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_started: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub installed_at: Option<OffsetDateTime>,
}

impl RuntimeManifest {
    pub fn new(
        runtime_version: impl Into<String>,
        backend: RuntimeBackend,
        platform: impl Into<String>,
        install_path: PathBuf,
    ) -> Self {
        Self {
            runtime_version: runtime_version.into(),
            backend,
            platform: platform.into(),
            install_path,
            installed: false,
            verified: false,
            sha256: None,
            last_started: None,
            installed_at: None,
        }
    }

    pub fn path(data_dir: &Path) -> PathBuf {
        data_dir.join("runtime").join("manifest.json")
    }

    pub async fn load(data_dir: &Path) -> RuntimeResult<Option<Self>> {
        let path = Self::path(data_dir);
        if !path.is_file() {
            return Ok(None);
        }
        let raw = tokio::fs::read_to_string(&path)
            .await
            .map_err(|err| RuntimeError::Config(err.to_string()))?;
        serde_json::from_str(&raw).map_err(|err| RuntimeError::Config(err.to_string()))
    }

    pub async fn save(&self, data_dir: &Path) -> RuntimeResult<()> {
        let path = Self::path(data_dir);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|err| RuntimeError::Config(err.to_string()))?;
        }
        let raw = serde_json::to_string_pretty(self)
            .map_err(|err| RuntimeError::Config(err.to_string()))?;
        tokio::fs::write(path, raw)
            .await
            .map_err(|err| RuntimeError::Config(err.to_string()))
    }
}
