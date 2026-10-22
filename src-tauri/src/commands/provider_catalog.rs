//! Best-effort provider model catalogs for the Add/Edit Model form.
//! A missing catalog or a failed HTTP call returns an empty list.

use std::collections::HashMap;
use std::time::Duration;

use promptlab_core::{build_http_client, HttpClientOptions};
use promptlab_harness::sigv4::{sign_headers, SigV4Credentials};
use serde::Serialize;
use serde_json::Value;
use time::OffsetDateTime;

const PAGE_LIMIT: usize = 5;
const MODEL_LIMIT: usize = 2_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelCatalogDto {
    pub models: Vec<String>,
    pub available: bool,
}

impl ProviderModelCatalogDto {
    pub fn from_models(models: Vec<String>) -> Self {
        let available = !models.is_empty();
        Self { models, available }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CatalogKind {
    Bearer,
    Anthropic,
    Gemini,
    Azure,
    Bedrock,
    YangYang,
}

pub async fn fetch_provider_models(
    provider: &str,
    base_url: Option<&str>,
    region: Option<&str>,
    api_key: &str,
    aws_secret: &str,
    aws_session: &str,
) -> Vec<String> {
    let kind = catalog_kind(provider);
    if kind == CatalogKind::Bedrock {
        if api_key.trim().is_empty() || aws_secret.trim().is_empty() {
            return Vec::new();
        }
    } else if catalog_requires_key(provider) && api_key.trim().is_empty() {
        return Vec::new();
    }

    let urls = catalog_urls(provider, base_url, region);
    if urls.is_empty() {
        return Vec::new();
    }

    let client = match build_http_client(
        HttpClientOptions::default()
            .with_timeout(Duration::from_secs(20))
            .with_connect_timeout(Duration::from_secs(8))
            .with_user_agent("PromptLab"),
    ) {
        Ok(client) => client,
        Err(err) => {
            tracing::debug!(error = %err, "provider catalog client unavailable");
            return Vec::new();
        }
    };

    let mut models = Vec::new();
    for url in urls {
        let page = fetch_pages(&client, kind, &url, api_key, aws_secret, aws_session, region).await;
        for id in page {
            push_model(&mut models, &id);
            if models.len() >= MODEL_LIMIT {
                return models;
            }
        }
    }
    models
}

pub(crate) fn catalog_urls(provider: &str, base_url: Option<&str>, region: Option<&str>) -> Vec<String> {
    let kind = catalog_kind(provider);
    if kind == CatalogKind::Bedrock {
        let region = region
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("us-east-1");
        return vec![
            format!("https://bedrock.{region}.amazonaws.com/foundation-models"),
            format!("https://bedrock.{region}.amazonaws.com/inference-profiles"),
        ];
    }

    let base = base_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| default_catalog_base(provider).map(str::to_string));
    let Some(base) = base else {
        return Vec::new();
    };

    match kind {
        CatalogKind::Azure => azure_deployments_url(&base).into_iter().collect(),
        CatalogKind::Gemini => vec![join_models(&base)],
        _ => vec![join_models(&base)],
    }
}

fn catalog_kind(provider: &str) -> CatalogKind {
    match provider.trim().to_ascii_lowercase().as_str() {
        "anthropic" => CatalogKind::Anthropic,
        "gemini" | "google" => CatalogKind::Gemini,
        "azure" => CatalogKind::Azure,
        "bedrock" | "aws_bedrock" => CatalogKind::Bedrock,
        "yangyang" | "yang_yang" => CatalogKind::YangYang,
        _ => CatalogKind::Bearer,
    }
}

fn catalog_requires_key(provider: &str) -> bool {
    matches!(
        provider.trim().to_ascii_lowercase().as_str(),
        "nvidia" | "openai" | "anthropic" | "gemini" | "google" | "openrouter" | "azure" | "yangyang"
    )
}

fn default_catalog_base(provider: &str) -> Option<&'static str> {
    match provider.trim().to_ascii_lowercase().as_str() {
        "openai" => Some("https://api.openai.com/v1"),
        "anthropic" => Some("https://api.anthropic.com/v1"),
        "gemini" | "google" => Some("https://generativelanguage.googleapis.com/v1beta"),
        "openrouter" => Some("https://openrouter.ai/api/v1"),
        "nvidia" => Some("https://integrate.api.nvidia.com/v1"),
        "yangyang" | "yang_yang" => Some("https://inference.yyng.icu/v1"),
        _ => None,
    }
}

fn join_models(base: &str) -> String {
    let mut base = base.trim().trim_end_matches('/').to_string();
    for suffix in ["/chat/completions", "/completions", "/messages", "/models"] {
        if let Some(stripped) = base.strip_suffix(suffix) {
            base = stripped.trim_end_matches('/').to_string();
            break;
        }
    }
    format!("{base}/models")
}

fn azure_deployments_url(base: &str) -> Option<String> {
    let Ok(mut url) = url::Url::parse(base.trim()) else {
        return None;
    };
    let path = url.path().to_string();
    let root = if let Some(idx) = path.find("/openai/deployments") {
        path[..idx + "/openai/deployments".len()].to_string()
    } else if path.is_empty() || path == "/" {
        "/openai/deployments".to_string()
    } else {
        format!("{}/openai/deployments", path.trim_end_matches('/'))
    };
    url.set_path(&root);
    url.set_query(Some("api-version=2024-10-21"));
    Some(url.to_string())
}

async fn fetch_pages(
    client: &reqwest::Client,
    kind: CatalogKind,
    start_url: &str,
    api_key: &str,
    aws_secret: &str,
    aws_session: &str,
    region: Option<&str>,
) -> Vec<String> {
    let mut models = Vec::new();
    let mut url = start_url.to_string();
    if kind == CatalogKind::Gemini {
        url = append_query(&url, "key", api_key.trim());
    }
    for _ in 0..PAGE_LIMIT {
        let Some(body) = get_body(client, kind, &url, api_key, aws_secret, aws_session, region).await else {
            break;
        };
        let value: Value = match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(_) => break,
        };
        for id in parse_model_ids(&value) {
            push_model(&mut models, &id);
        }
        let Some(next) = next_page_url(kind, &url, &value) else {
            break;
        };
        url = next;
    }
    models
}

async fn get_body(
    client: &reqwest::Client,
    kind: CatalogKind,
    url: &str,
    api_key: &str,
    aws_secret: &str,
    aws_session: &str,
    region: Option<&str>,
) -> Option<String> {
    let mut headers = HashMap::new();
    match kind {
        CatalogKind::Bearer => {
            let key = api_key.trim();
            if !key.is_empty() {
                headers.insert("Authorization".into(), format!("Bearer {key}"));
            }
        }
        CatalogKind::Anthropic => {
            headers.insert("x-api-key".into(), api_key.trim().to_string());
            headers.insert("anthropic-version".into(), "2023-06-01".into());
        }
        CatalogKind::Azure => {
            headers.insert("api-key".into(), api_key.trim().to_string());
        }
        CatalogKind::YangYang => {
            headers.insert(
                "x-yang-api-token".into(),
                yang_token_header(api_key),
            );
        }
        CatalogKind::Gemini => {}
        CatalogKind::Bedrock => {
            let parsed = url::Url::parse(url).ok()?;
            let region = region
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("us-east-1");
            sign_headers(
                "GET",
                &parsed,
                &mut headers,
                "",
                SigV4Credentials {
                    access_key_id: api_key.trim(),
                    secret_access_key: aws_secret.trim(),
                    session_token: Some(aws_session.trim()).filter(|value| !value.is_empty()),
                    region,
                    service: "bedrock",
                },
                OffsetDateTime::now_utc(),
            )
            .ok()?;
        }
    }

    let mut request = client.get(url);
    for (name, value) in headers {
        request = request.header(name, value);
    }
    let response = match request.send().await {
        Ok(response) => response,
        Err(err) => {
            tracing::debug!(error = %err, "provider catalog request failed");
            return None;
        }
    };
    if !response.status().is_success() {
        tracing::debug!(status = %response.status(), "provider catalog unavailable");
        return None;
    }
    response.text().await.ok()
}

fn next_page_url(kind: CatalogKind, current: &str, body: &Value) -> Option<String> {
    match kind {
        CatalogKind::Gemini => {
            let token = body.get("nextPageToken").and_then(|value| value.as_str())?;
            if token.is_empty() {
                return None;
            }
            Some(append_query(current, "pageToken", token))
        }
        CatalogKind::Anthropic => {
            if body.get("has_more").and_then(|value| value.as_bool()) != Some(true) {
                return None;
            }
            let last = body.get("last_id").and_then(|value| value.as_str())?;
            if last.is_empty() {
                return None;
            }
            Some(append_query(current, "after_id", last))
        }
        _ => None,
    }
}

fn append_query(url: &str, key: &str, value: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(url) else {
        return url.to_string();
    };
    let kept: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(name, _)| name != key)
        .map(|(name, item)| (name.into_owned(), item.into_owned()))
        .collect();
    parsed.set_query(None);
    {
        let mut query = parsed.query_pairs_mut();
        for (name, item) in &kept {
            query.append_pair(name, item);
        }
        query.append_pair(key, value);
    }
    parsed.to_string()
}

pub(crate) fn parse_model_ids(body: &Value) -> Vec<String> {
    let mut models = Vec::new();
    if let Some(items) = body.get("data").and_then(|value| value.as_array()) {
        for item in items {
            if let Some(id) = item.get("id").and_then(|value| value.as_str()) {
                push_model(&mut models, id);
            }
        }
    }
    if let Some(items) = body.get("models").and_then(|value| value.as_array()) {
        for item in items {
            if !gemini_can_generate(item) {
                continue;
            }
            let id = item
                .get("name")
                .and_then(|value| value.as_str())
                .or_else(|| item.get("id").and_then(|value| value.as_str()));
            if let Some(id) = id {
                push_model(&mut models, id);
            }
        }
    }
    if let Some(items) = body.get("modelSummaries").and_then(|value| value.as_array()) {
        for item in items {
            if let Some(id) = item.get("modelId").and_then(|value| value.as_str()) {
                push_model(&mut models, id);
            }
        }
    }
    if let Some(items) = body
        .get("inferenceProfileSummaries")
        .and_then(|value| value.as_array())
    {
        for item in items {
            if let Some(id) = item
                .get("inferenceProfileId")
                .and_then(|value| value.as_str())
            {
                push_model(&mut models, id);
            }
        }
    }
    models
}

fn gemini_can_generate(item: &Value) -> bool {
    let Some(methods) = item.get("supportedGenerationMethods").and_then(|value| value.as_array()) else {
        return true;
    };
    methods.iter().any(|method| method.as_str() == Some("generateContent"))
}

fn push_model(models: &mut Vec<String>, id: &str) {
    let id = id.trim().strip_prefix("models/").unwrap_or(id.trim());
    if id.is_empty() || models.iter().any(|existing| existing == id) {
        return;
    }
    if models.len() >= MODEL_LIMIT {
        return;
    }
    models.push(id.to_string());
}

fn yang_token_header(api_key: &str) -> String {
    let trimmed = api_key.trim();
    if trimmed.len() >= 6 && trimmed[..6].eq_ignore_ascii_case("basic ") {
        format!("Basic {}", trimmed[6..].trim())
    } else {
        format!("Basic {trimmed}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_url_uses_default_base() {
        assert_eq!(
            catalog_urls("openai", None, None),
            vec!["https://api.openai.com/v1/models".to_string()]
        );
    }

    #[test]
    fn custom_base_strips_chat_path() {
        assert_eq!(
            catalog_urls(
                "lm-studio",
                Some("https://host/v1/chat/completions"),
                None
            ),
            vec!["https://host/v1/models".to_string()]
        );
    }

    #[test]
    fn azure_url_lists_deployments() {
        assert_eq!(
            catalog_urls(
                "azure",
                Some("https://res.openai.azure.com/openai/deployments/gpt-4o"),
                None
            ),
            vec!["https://res.openai.azure.com/openai/deployments?api-version=2024-10-21".to_string()]
        );
    }

    #[test]
    fn parse_openai_and_gemini_and_bedrock() {
        let openai = serde_json::json!({"data": [{"id": "gpt-4o"}, {"id": "gpt-4o-mini"}]});
        assert_eq!(parse_model_ids(&openai), vec!["gpt-4o", "gpt-4o-mini"]);

        let gemini = serde_json::json!({
            "models": [
                {"name": "models/gemini-2.0-flash", "supportedGenerationMethods": ["generateContent"]},
                {"name": "models/text-embedding-004", "supportedGenerationMethods": ["embedContent"]}
            ]
        });
        assert_eq!(parse_model_ids(&gemini), vec!["gemini-2.0-flash"]);

        let bedrock = serde_json::json!({
            "modelSummaries": [{"modelId": "anthropic.claude-3-haiku-20240307-v1:0"}],
            "inferenceProfileSummaries": [{"inferenceProfileId": "us.anthropic.claude-3-5-haiku-20241022-v1:0"}]
        });
        assert_eq!(
            parse_model_ids(&bedrock),
            vec![
                "anthropic.claude-3-haiku-20240307-v1:0",
                "us.anthropic.claude-3-5-haiku-20241022-v1:0"
            ]
        );
    }
}
