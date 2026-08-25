//! Model discovery stubs — local GGUF vault scanning removed (remote-only).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::RuntimeResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredModel {
    pub name: String,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
    pub digest: Option<String>,
}

/// Local GGUF vault discovery removed — always empty.
pub async fn discover_models_in_dir(_models_dir: &Path) -> RuntimeResult<Vec<DiscoveredModel>> {
    Ok(Vec::new())
}

pub async fn check_health(_base_url: Option<&str>, _models_dir: Option<&Path>) -> RuntimeResult<bool> {
    Ok(false)
}

pub async fn discover_models(models_dir: &Path) -> RuntimeResult<Vec<DiscoveredModel>> {
    discover_models_in_dir(models_dir).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn discovery_always_empty() {
        let dir = tempfile::tempdir().unwrap();
        let models = discover_models_in_dir(dir.path()).await.unwrap();
        assert!(models.is_empty());
    }
}
