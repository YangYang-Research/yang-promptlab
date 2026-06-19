use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::debug;

use crate::error::{RuntimeError, RuntimeResult};
use crate::runtime::gguf::detect_quantization;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredModel {
    pub name: String,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
    pub digest: Option<String>,
}

/// Scan the model vault directory for `.gguf` files.
pub async fn discover_models_in_dir(models_dir: &Path) -> RuntimeResult<Vec<DiscoveredModel>> {
    if !models_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut discovered = Vec::new();
    let mut stack = vec![models_dir.to_path_buf()];

    while let Some(current) = stack.pop() {
        let mut entries = fs::read_dir(&current)
            .await
            .map_err(|err| RuntimeError::Process(err.to_string()))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|err| RuntimeError::Process(err.to_string()))?
        {
            let path = entry.path();
            let file_type = entry
                .file_type()
                .await
                .map_err(|err| RuntimeError::Process(err.to_string()))?;

            if file_type.is_dir() {
                stack.push(path);
                continue;
            }

            if path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("gguf"))
                != Some(true)
            {
                continue;
            }

            let metadata = entry
                .metadata()
                .await
                .map_err(|err| RuntimeError::Process(err.to_string()))?;

            let name = path
                .strip_prefix(models_dir)
                .ok()
                .and_then(|p| p.to_str())
                .unwrap_or_else(|| {
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("model.gguf")
                })
                .to_string();

            let quant = detect_quantization(&path);
            let digest = Some(format!("gguf:{}", quant.as_str()));

            debug!(path = %path.display(), quant = quant.as_str(), "discovered gguf model");

            discovered.push(DiscoveredModel {
                name,
                size_bytes: metadata.len(),
                modified_at: None,
                digest,
            });
        }
    }

    discovered.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(discovered)
}

/// Probe llama-server health via `/health`, or report vault availability when idle.
pub async fn check_health(base_url: Option<&str>, models_dir: Option<&Path>) -> RuntimeResult<bool> {
    let default_url = crate::config::default_llama_base_url();
    let base = base_url
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(default_url.as_str());

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|err| RuntimeError::Process(err.to_string()))?;

    let url = format!("{}/health", base.trim_end_matches('/'));
    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => return Ok(true),
        Ok(_) => return Ok(false),
        Err(err) => {
            debug!(%err, "llama.cpp health probe failed");
        }
    }

    // Idle supervisor: healthy when vault contains at least one GGUF file.
    if let Some(dir) = models_dir {
        let models = discover_models_in_dir(dir).await?;
        return Ok(!models.is_empty());
    }

    Ok(false)
}

/// Backward-compatible wrapper — lists GGUF models from the vault at `models_dir`.
pub async fn discover_models(
    models_dir: &Path,
) -> RuntimeResult<Vec<DiscoveredModel>> {
    discover_models_in_dir(models_dir).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_dir_returns_no_models() {
        let dir = tempfile::tempdir().unwrap();
        let models = discover_models_in_dir(dir.path()).await.unwrap();
        assert!(models.is_empty());
    }
}
