use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::config::default_ollama_base_url;
use crate::error::{RuntimeError, RuntimeResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredModel {
    pub name: String,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
    pub digest: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<TagModel>,
}

#[derive(Debug, Deserialize)]
struct TagModel {
    name: String,
    #[serde(default)]
    size: u64,
    #[serde(default, rename = "modified_at")]
    modified_at: Option<String>,
    #[serde(default)]
    digest: Option<String>,
}

/// List models installed in a running Ollama instance (`GET /api/tags`).
pub async fn discover_models(base_url: Option<&str>) -> RuntimeResult<Vec<DiscoveredModel>> {
    let default_url = default_ollama_base_url();
    let base = base_url
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(default_url.as_str());
    let url = format!("{}/api/tags", base.trim_end_matches('/'));

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|err| RuntimeError::Process(err.to_string()))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|err| RuntimeError::Process(format!("ollama tags request failed: {err}")))?;

    if !response.status().is_success() {
        return Err(RuntimeError::Process(format!(
            "ollama tags returned {}",
            response.status()
        )));
    }

    let body: TagsResponse = response
        .json()
        .await
        .map_err(|err| RuntimeError::Process(format!("ollama tags parse failed: {err}")))?;

    Ok(body
        .models
        .into_iter()
        .map(|m| DiscoveredModel {
            name: m.name,
            size_bytes: m.size,
            modified_at: m.modified_at,
            digest: m.digest,
        })
        .collect())
}

/// Probe Ollama health via `/api/tags`.
pub async fn check_health(base_url: Option<&str>) -> RuntimeResult<bool> {
    discover_models(base_url).await.map(|_| true).or_else(|err| {
        if matches!(err, RuntimeError::Unavailable) {
            Ok(false)
        } else {
            Ok(false)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tags_payload() {
        let raw = r#"{"models":[{"name":"llama3:latest","size":4661211424,"digest":"abc"}]}"#;
        let parsed: TagsResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.models[0].name, "llama3:latest");
        assert_eq!(parsed.models[0].size, 4661211424);
    }
}
