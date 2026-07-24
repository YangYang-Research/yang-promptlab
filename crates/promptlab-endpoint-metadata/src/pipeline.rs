use std::collections::HashMap;
use std::time::Duration;

use promptlab_fingerprint::{FingerprintEngine, FingerprintInput, StackFingerprintReport};
use time::OffsetDateTime;
use url::Url;

use crate::capability::CapabilityDetector;
use crate::classify::EndpointClassifier;
use crate::risk::RiskScorer;
use crate::schema::SchemaInferenceEngine;
use crate::types::{
    AiEndpointMetadata, DiscoveryProvenance, EndpointBasic, EndpointClassification,
    EndpointCapabilities, FingerprintMetadata, InferenceFields, RawObservation, SchemaMetadata,
};

/// Input for a single endpoint analysis pass.
#[derive(Debug, Clone)]
pub struct DiscoveryAnalysisInput {
    pub endpoint_id: String,
    pub url: String,
    pub method: String,
    pub kind: String,
    pub discovery_confidence: f64,
    pub discovery_source: String,
    pub evidence: Option<String>,
    pub discovered_at: OffsetDateTime,
    pub auth_required: bool,
}

/// Probe HTTP and run the full metadata pipeline for one endpoint.
pub async fn analyze_endpoint(
    client: &reqwest::Client,
    input: DiscoveryAnalysisInput,
) -> AiEndpointMetadata {
    let (status, headers, content_type, response_body) = probe_endpoint(client, &input.url, &input.method).await;

    let host = Url::parse(&input.url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
    let protocol = Url::parse(&input.url)
        .map(|u| u.scheme().to_string())
        .unwrap_or_else(|_| "https".into());

    let fp_input = FingerprintInput::from_snapshot(
        input.url.clone(),
        Some(input.method.clone()),
        status,
        headers.clone(),
        content_type.clone(),
        response_body.clone().unwrap_or_default(),
        Some(input.kind.clone()),
    );

    let stack_report = FingerprintEngine::new().fingerprint_stack(&fp_input);
    let fingerprint = fingerprint_from_report(&stack_report);

    let request_body = default_probe_body(&input.url, &input.method, &fingerprint.api_style);
    let (schema, inference) = SchemaInferenceEngine::infer(
        &input.url,
        &input.method,
        content_type.as_deref(),
        Some(request_body.as_str()),
        response_body.as_deref(),
        &fingerprint.api_style,
    );

    let capabilities = CapabilityDetector::detect(
        &input.url,
        &fingerprint,
        &schema,
        stack_report.platform_profile.tools_enabled,
        stack_report.platform_profile.memory_enabled,
        stack_report
            .ai_components
            .iter()
            .any(|c| c.component == promptlab_fingerprint::AiComponent::McpServer),
    );

    let mut classification = EndpointClassifier::classify(
        &input.url,
        &input.kind,
        &fingerprint,
        &capabilities,
        input.discovery_confidence,
    );

    let risk = RiskScorer::score(
        &classification,
        &capabilities,
        input.auth_required,
        !input.auth_required,
    );
    classification.risk_score = risk.score;

    AiEndpointMetadata {
        basic: EndpointBasic {
            id: input.endpoint_id,
            url: input.url,
            method: input.method,
            host,
            protocol,
            status,
        },
        fingerprint,
        schema,
        inference,
        capabilities,
        classification,
        risk,
        provenance: DiscoveryProvenance {
            discovery_source: input.discovery_source,
            authentication_required: input.auth_required,
            discovered_at: input.discovered_at,
            kind: input.kind,
            evidence: input.evidence,
        },
        raw: Some(RawObservation {
            request_headers: HashMap::new(),
            request_body: Some(request_body),
            response_headers: headers,
            response_body,
        }),
        stack_fingerprint: Some(stack_report),
    }
}

/// Analyze many endpoints concurrently (bounded parallelism).
pub async fn analyze_endpoints_batch(
    client: &reqwest::Client,
    inputs: Vec<DiscoveryAnalysisInput>,
    concurrency: usize,
) -> Vec<AiEndpointMetadata> {
    use tokio::sync::Semaphore;
    use tokio::task::JoinSet;

    let sem = std::sync::Arc::new(Semaphore::new(concurrency.max(1)));
    let client = std::sync::Arc::new(client.clone());
    let mut set = JoinSet::new();

    for input in inputs {
        let permit = sem.clone().acquire_owned().await.expect("semaphore");
        let client = client.clone();
        set.spawn(async move {
            let result = analyze_endpoint(&client, input).await;
            drop(permit);
            result
        });
    }

    let mut results = Vec::new();
    while let Some(joined) = set.join_next().await {
        if let Ok(metadata) = joined {
            results.push(metadata);
        }
    }
    results
}

async fn probe_endpoint(
    client: &reqwest::Client,
    url: &str,
    method: &str,
) -> (u16, HashMap<String, String>, Option<String>, Option<String>) {
    let method = method.to_ascii_uppercase();
    let mut request = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            url,
        )
        .timeout(Duration::from_secs(10));

    if method == "POST" || method == "PUT" || method == "PATCH" {
        request = request
            .header("Content-Type", "application/json")
            .body(default_probe_body(url, &method, "openai_compatible"));
    }

    match request.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let headers: HashMap<String, String> = resp
                .headers()
                .iter()
                .filter_map(|(k, v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string())))
                .collect();
            let content_type = headers.get("content-type").cloned();
            let body = resp.text().await.ok();
            (status, headers, content_type, body)
        }
        Err(_) => (0, HashMap::new(), None, None),
    }
}

fn default_probe_body(url: &str, method: &str, api_style: &str) -> String {
    if !method.eq_ignore_ascii_case("POST") && !method.eq_ignore_ascii_case("PUT") {
        return String::new();
    }
    let path = url.to_ascii_lowercase();
    if path.contains("embedding") {
        return r#"{"model":"probe","input":"ping"}"#.into();
    }
    if api_style == "anthropic_messages" {
        return r#"{"model":"probe","max_tokens":8,"messages":[{"role":"user","content":"ping"}]}"#.into();
    }
    r#"{"model":"probe","messages":[{"role":"user","content":"ping"}]}"#.into()
}

fn fingerprint_from_report(report: &StackFingerprintReport) -> FingerprintMetadata {
    let primary = report.provider_report.primary.as_ref();
    let framework = if !report.platform_profile.platform.is_empty() {
        report.platform_profile.platform.clone()
    } else {
        report
            .technologies
            .first()
            .map(|t| t.name.clone())
            .unwrap_or_else(|| "unknown".into())
    };

    FingerprintMetadata {
        framework: framework.clone(),
        provider: primary
            .map(|p| p.provider.as_str().to_string())
            .unwrap_or_else(|| framework),
        version: report.platform_profile.version.clone(),
        confidence: report.confidence,
        api_style: primary
            .map(|p| api_style_str(p.inferred_api_style))
            .unwrap_or_else(|| "unknown".into()),
        technologies: report.technologies.iter().map(|t| t.name.clone()).collect(),
    }
}

fn api_style_str(style: promptlab_fingerprint::ApiStyle) -> String {
    match style {
        promptlab_fingerprint::ApiStyle::OpenAiCompatible => "openai_compatible",
        promptlab_fingerprint::ApiStyle::AnthropicMessages => "anthropic_messages",
        promptlab_fingerprint::ApiStyle::GeminiGenerateContent => "gemini_generate_content",
        promptlab_fingerprint::ApiStyle::BedrockInvoke => "bedrock_invoke",
        promptlab_fingerprint::ApiStyle::OllamaNative => "ollama_native",
        promptlab_fingerprint::ApiStyle::Unknown => "unknown",
    }
    .into()
}

/// Build request body template with `{{payload}}` placeholder in inferred fields.
pub fn body_template_from_metadata(metadata: &AiEndpointMetadata) -> String {
    let inference = &metadata.inference;
    let mut root = serde_json::Map::new();

    if let Some(model_field) = &inference.model_field {
        root.insert(model_field.clone(), serde_json::json!("probe"));
    } else {
        root.insert("model".into(), serde_json::json!("probe"));
    }

    if let Some(stream_field) = &inference.stream_field {
        root.insert(stream_field.clone(), serde_json::json!(false));
    }

    let prompt_injection = serde_json::json!([{"role":"user","content":"{{payload}}"}]);

    if let Some(history_field) = &inference.history_field {
        root.insert(history_field.clone(), prompt_injection);
    } else if let Some(prompt_field) = &inference.prompt_field {
        root.insert(prompt_field.clone(), serde_json::json!("{{payload}}"));
    } else {
        root.insert("messages".into(), prompt_injection);
    }

    serde_json::Value::Object(root).to_string()
}

/// Build a JSON request body by injecting payload into inferred fields only.
pub fn build_payload_body(metadata: &AiEndpointMetadata, payload_content: &str) -> String {
    let inference = &metadata.inference;
    let mut root = serde_json::Map::new();

    if let Some(model_field) = &inference.model_field {
        root.insert(model_field.clone(), serde_json::json!("probe"));
    } else {
        root.insert("model".into(), serde_json::json!("probe"));
    }

    if let Some(stream_field) = &inference.stream_field {
        root.insert(stream_field.clone(), serde_json::json!(false));
    }

    let prompt_injection = serde_json::json!([{"role":"user","content": payload_content}]);

    if let Some(history_field) = &inference.history_field {
        root.insert(history_field.clone(), prompt_injection);
    } else if let Some(prompt_field) = &inference.prompt_field {
        root.insert(prompt_field.clone(), serde_json::json!(payload_content));
    } else {
        root.insert("messages".into(), prompt_injection);
    }

    serde_json::Value::Object(root).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EndpointType;

    #[test]
    fn build_payload_uses_inference_fields() {
        let mut meta = AiEndpointMetadata {
            basic: EndpointBasic {
                id: "1".into(),
                url: "https://x/v1/chat".into(),
                method: "POST".into(),
                host: "x".into(),
                protocol: "https".into(),
                status: 200,
            },
            fingerprint: FingerprintMetadata {
                framework: "openai".into(),
                provider: "openai".into(),
                version: String::new(),
                confidence: 0.9,
                api_style: "openai_compatible".into(),
                technologies: vec![],
            },
            schema: SchemaMetadata::default(),
            inference: InferenceFields {
                history_field: Some("messages".into()),
                model_field: Some("model".into()),
                ..Default::default()
            },
            capabilities: EndpointCapabilities::default(),
            classification: EndpointClassification {
                endpoint_type: EndpointType::AiChat,
                ai_framework: "openai".into(),
                confidence: 0.9,
                risk_score: 50,
            },
            risk: crate::types::RiskAssessment {
                score: 50,
                factors: vec![],
            },
            provenance: DiscoveryProvenance {
                discovery_source: "discovery".into(),
                authentication_required: false,
                discovered_at: OffsetDateTime::now_utc(),
                kind: "ai_endpoint".into(),
                evidence: None,
            },
            raw: None,
            stack_fingerprint: None,
        };

        let body = build_payload_body(&meta, "INJECT");
        assert!(body.contains("INJECT"));
        assert!(body.contains("messages"));
    }
}
