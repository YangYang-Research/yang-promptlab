//! Engine integration helpers for discovery, attack, and judge plugins.

use serde::{Deserialize, Serialize};

use crate::error::PluginResult;
use crate::manager::PluginManager;
use crate::types::{PluginInvokeResult, PluginType};

/// Endpoint suggested by a discovery plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDiscoveryEndpoint {
    pub url: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub method: Option<String>,
}

/// Supplemental judge signal from a plugin hook.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginJudgeSignal {
    pub plugin_id: String,
    pub vulnerable: bool,
    #[serde(default)]
    pub confidence: f32,
    #[serde(default)]
    pub summary: String,
}

/// Run all enabled plugins of a given type.
pub async fn invoke_enabled_by_type(
    manager: &mut PluginManager,
    plugin_type: PluginType,
    params: serde_json::Value,
) -> PluginResult<Vec<PluginInvokeResult>> {
    let ids: Vec<String> = manager
        .by_type(plugin_type)
        .into_iter()
        .filter(|record| record.enabled)
        .map(|record| record.id.clone())
        .collect();

    let mut results = Vec::with_capacity(ids.len());
    for id in ids {
        results.push(manager.invoke(&id, params.clone()).await?);
    }
    Ok(results)
}

/// Collect candidate endpoints from enabled discovery plugins.
pub async fn collect_discovery_endpoints(
    manager: &mut PluginManager,
    seed_url: &str,
) -> PluginResult<Vec<PluginDiscoveryEndpoint>> {
    let results = invoke_enabled_by_type(
        manager,
        PluginType::Discovery,
        serde_json::json!({ "target_url": seed_url, "url": seed_url }),
    )
    .await?;

    let mut endpoints = Vec::new();
    for result in results {
        if let Some(items) = result.result.get("endpoints").and_then(|v| v.as_array()) {
            for item in items {
                if let Ok(parsed) = serde_json::from_value::<PluginDiscoveryEndpoint>(item.clone()) {
                    if !parsed.url.trim().is_empty() {
                        endpoints.push(parsed);
                    }
                }
            }
        }
    }
    Ok(endpoints)
}

/// Mutate an attack payload through enabled attack plugins (in registration order).
pub async fn mutate_attack_payload(
    manager: &mut PluginManager,
    payload: &str,
) -> PluginResult<String> {
    let ids: Vec<String> = manager
        .by_type(PluginType::Attack)
        .into_iter()
        .filter(|record| record.enabled)
        .map(|record| record.id.clone())
        .collect();

    let mut current = payload.to_string();
    for id in ids {
        let result = manager
            .invoke(&id, serde_json::json!({ "payload": current }))
            .await?;
        if let Some(next) = result.result.get("payload").and_then(|v| v.as_str()) {
            current = next.to_string();
        }
    }
    Ok(current)
}

/// Evaluate response text with enabled judge plugins.
pub async fn evaluate_with_judge_plugins(
    manager: &mut PluginManager,
    response_text: &str,
    category: &str,
) -> PluginResult<Vec<PluginJudgeSignal>> {
    let results = invoke_enabled_by_type(
        manager,
        PluginType::Judge,
        serde_json::json!({
            "response_text": response_text,
            "category": category,
        }),
    )
    .await?;

    let mut signals = Vec::new();
    for result in results {
        let vulnerable = result
            .result
            .get("vulnerable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let confidence = result
            .result
            .get("confidence")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(0.5);
        let summary = result
            .result
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("plugin judge signal")
            .to_string();
        signals.push(PluginJudgeSignal {
            plugin_id: result.plugin_id,
            vulnerable,
            confidence,
            summary,
        });
    }
    Ok(signals)
}
