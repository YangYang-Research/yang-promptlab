//! Runs the AI-aware discovery metadata pipeline after endpoint enumeration.

use std::sync::Arc;
use std::time::Instant;

use aisec_discovery::types::DiscoveredEndpoint;
use aisec_endpoint_metadata::{
    analyze_endpoints_batch, DiscoveryAnalysisInput, AiEndpointMetadata,
};
use aisec_storage::CreateEndpoint;
use time::OffsetDateTime;
use tracing::info;

use crate::session_auth::resolve_discovery_auth;

pub const DISCOVERY_PIPELINE_PHASES: &[&str] = &[
    "discovering_endpoints",
    "fingerprinting",
    "inferring_schemas",
    "detecting_capabilities",
    "classifying_endpoints",
    "calculating_risk",
    "saving_metadata",
];

pub struct PipelineProgress {
    pub phase: String,
    pub processed: usize,
    pub total: usize,
    pub elapsed_ms: u64,
}

pub async fn build_metadata_for_discovered(
    client: &reqwest::Client,
    discovered: &[DiscoveredEndpoint],
    target_id: &str,
    scan_id: &str,
    auth_required: bool,
    concurrency: usize,
    mut on_progress: impl FnMut(PipelineProgress),
) -> Vec<CreateEndpoint> {
    let started = Instant::now();
    let total = discovered.len();

    on_progress(PipelineProgress {
        phase: DISCOVERY_PIPELINE_PHASES[1].into(),
        processed: 0,
        total,
        elapsed_ms: 0,
    });

    let inputs: Vec<DiscoveryAnalysisInput> = discovered
        .iter()
        .map(|e| {
            let endpoint_id = aisec_storage::util::new_id();
            let source = discovery_source_label(e);
            DiscoveryAnalysisInput {
                endpoint_id,
                url: e.url.clone(),
                method: e
                    .method
                    .clone()
                    .unwrap_or_else(|| "GET".into()),
                kind: e.kind.as_str().to_string(),
                discovery_confidence: e.confidence as f64,
                discovery_source: source,
                evidence: Some(e.evidence.clone()),
                discovered_at: e.discovered_at,
                auth_required,
            }
        })
        .collect();

    on_progress(PipelineProgress {
        phase: DISCOVERY_PIPELINE_PHASES[2].into(),
        processed: 0,
        total,
        elapsed_ms: started.elapsed().as_millis() as u64,
    });

    let metadata_list = analyze_endpoints_batch(client, inputs, concurrency).await;

    on_progress(PipelineProgress {
        phase: DISCOVERY_PIPELINE_PHASES[5].into(),
        processed: metadata_list.len(),
        total,
        elapsed_ms: started.elapsed().as_millis() as u64,
    });

    let mut creates = Vec::with_capacity(metadata_list.len());
    for metadata in metadata_list {
        if let Some(create) = metadata_to_create_endpoint(metadata, scan_id, target_id) {
            creates.push(create);
        }
    }

    on_progress(PipelineProgress {
        phase: DISCOVERY_PIPELINE_PHASES[6].into(),
        processed: creates.len(),
        total,
        elapsed_ms: started.elapsed().as_millis() as u64,
    });

    info!(
        endpoints = creates.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "metadata pipeline complete"
    );

    creates
}

fn metadata_to_create_endpoint(
    metadata: AiEndpointMetadata,
    scan_id: &str,
    target_id: &str,
) -> Option<CreateEndpoint> {
    let json = metadata.to_json().ok()?;
    Some(CreateEndpoint {
        scan_id: scan_id.into(),
        target_id: Some(target_id.into()),
        url: metadata.basic.url.clone(),
        kind: metadata.provenance.kind.clone(),
        method: Some(metadata.basic.method.clone()),
        confidence: metadata.classification.confidence as f64,
        evidence: metadata.provenance.evidence.clone(),
        source_url: Some(metadata.provenance.discovery_source.clone()),
        discovered_at: metadata.provenance.discovered_at,
        metadata_json: Some(json),
        endpoint_type: Some(metadata.classification.endpoint_type.as_str().into()),
        ai_framework: Some(metadata.classification.ai_framework.clone()),
        risk_score: Some(metadata.risk.score as i64),
        metadata_confidence: Some(metadata.classification.confidence as f64),
        discovery_source: Some(metadata.provenance.discovery_source.clone()),
        auth_required: Some(metadata.provenance.authentication_required),
    })
}

fn discovery_source_label(endpoint: &DiscoveredEndpoint) -> String {
    if endpoint.source_url.as_deref() == Some("manual") {
        "manual".into()
    } else if endpoint.evidence.contains("plugin") {
        "plugin".into()
    } else {
        "discovery".into()
    }
}

pub async fn target_requires_auth(
    state: &crate::state::AppState,
    descriptor_json: &str,
    seed_url: &str,
) -> bool {
    resolve_discovery_auth(state, descriptor_json, seed_url)
        .await
        .ok()
        .flatten()
        .is_some()
}

pub fn analysis_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .expect("analysis http client")
}

pub fn analysis_concurrency() -> usize {
    6
}
