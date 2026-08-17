//! AI-backed remediation recommendations for a single finding.
//!
//! Uses a dedicated finding-remediation system prompt (not scan Recommendations).
//! Cache lives in finding `evidence_json.recommendations`; regenerate on fingerprint
//! miss or `force`, with rule-based fallback when AI is unavailable.

use promptlab_report::{
    data::StorageFindingRow, generate_recommendations, recommendation_for, Severity,
};
use promptlab_storage::{Finding, FindingRepository, UpdateFinding};
use promptlab_target_profile::{
    generate_finding_recommendations_with_llm, AttackRecommendation, AttackRecommendationsBundle,
    FindingRemediationInput,
};
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::warn;

use crate::error::CommandResult;
use crate::inference_host::{is_inference_ready, HostFindingRecommendLlm};
use crate::state::AppState;

const EVIDENCE_RECOMMENDATIONS_KEY: &str = "recommendations";
/// Bump when the finding-remediation system prompt changes so stale caches regenerate.
const FINDING_REMEDIATION_PROMPT_VERSION: &str = "finding-remediation-v1";

#[derive(Debug, Deserialize)]
pub struct FindingRecommendationsRequest {
    pub finding_id: String,
    /// When true, regenerate and overwrite any cached recommendations in finding evidence.
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingRecommendationDto {
    pub title: String,
    pub description: String,
    pub priority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredFindingRecommendations {
    overview: String,
    source: String,
    recommendations: Vec<FindingRecommendationDto>,
    #[serde(default)]
    generated_at: String,
    /// Hash of finding fields that should invalidate stale remediation advice.
    #[serde(default)]
    input_fingerprint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingRecommendationsResponse {
    pub source: String,
    pub overview: String,
    pub recommendations: Vec<FindingRecommendationDto>,
    pub generated_at: String,
}

pub async fn finding_recommendations_generate_op(
    state: &AppState,
    request: FindingRecommendationsRequest,
) -> CommandResult<FindingRecommendationsResponse> {
    let repos = state.repositories();
    let finding = repos
        .findings()
        .get(&request.finding_id)
        .await
        .map_err(crate::error::CommandError::from)?;

    let input_fingerprint = finding_recommendations_fingerprint(&finding);

    if !request.force {
        if let Some(cached) = load_stored_recommendations(finding.evidence_json.as_deref()) {
            if !cached.input_fingerprint.is_empty()
                && cached.input_fingerprint == input_fingerprint
            {
                return Ok(FindingRecommendationsResponse {
                    source: cached.source,
                    overview: cached.overview,
                    recommendations: cached.recommendations,
                    generated_at: cached.generated_at,
                });
            }
        }
    }

    let remediation_input = finding_remediation_input(&finding);

    let bundle = {
        let inference = state.inference_manager().lock().await;
        if is_inference_ready(&inference) {
            drop(inference);
            let llm = HostFindingRecommendLlm::new(
                state.data_dir().to_path_buf(),
                state.inference_manager().clone(),
                state.model_manager().clone(),
                state.model_provider().clone(),
                state.runtime_manager().clone(),
            );
            match generate_finding_recommendations_with_llm(&remediation_input, &llm).await {
                Ok(bundle) => Some(("ai", bundle)),
                Err(err) => {
                    warn!(
                        finding_id = %request.finding_id,
                        error = %err,
                        "AI finding recommendations failed; using rule-based fallback"
                    );
                    None
                }
            }
        } else {
            None
        }
    };

    let (source, bundle) = match bundle {
        Some((source, bundle)) => (source, bundle),
        None => ("fallback", fallback_finding_recommendations(&finding)),
    };

    let generated_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".into());

    let response = FindingRecommendationsResponse {
        source: source.into(),
        overview: bundle.overview,
        recommendations: bundle
            .recommendations
            .into_iter()
            .map(|item| FindingRecommendationDto {
                title: item.title,
                description: item.description,
                priority: item.priority,
            })
            .collect(),
        generated_at,
    };

    if let Err(err) =
        persist_recommendations(&repos, &finding, &response, &input_fingerprint).await
    {
        warn!(
            finding_id = %request.finding_id,
            error = %err,
            "failed to persist finding recommendations"
        );
    }

    Ok(response)
}

fn finding_remediation_input(finding: &Finding) -> FindingRemediationInput {
    let evidence = finding
        .evidence_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .map(|mut value| {
            // Recommendations cache is write-side metadata; do not feed it back into the LLM.
            if let Some(obj) = value.as_object_mut() {
                obj.remove(EVIDENCE_RECOMMENDATIONS_KEY);
            }
            value
        });

    FindingRemediationInput {
        finding_id: finding.id.clone(),
        title: finding.title.clone(),
        severity: finding.severity.clone(),
        category: finding
            .category
            .clone()
            .unwrap_or_else(|| "unknown".into()),
        description: finding.description.clone(),
        status: finding.status.clone(),
        evidence,
    }
}

fn fallback_finding_recommendations(finding: &Finding) -> AttackRecommendationsBundle {
    let category = finding.category.as_deref().unwrap_or("unknown");
    let severity = Severity::from_str_loose(&finding.severity);
    let primary = recommendation_for(category, severity);

    let report_row = StorageFindingRow {
        id: finding.id.clone(),
        title: finding.title.clone(),
        severity: finding.severity.clone(),
        category: finding.category.clone(),
        description: finding.description.clone(),
        evidence_json: finding.evidence_json.clone(),
        status: finding.status.clone(),
    };
    let report_finding = report_row.into_report_finding();

    let mut recommendations = vec![AttackRecommendation {
        title: primary.title,
        description: primary.description,
        priority: finding.severity.to_ascii_lowercase(),
        action: None,
    }];

    for rec in generate_recommendations(std::slice::from_ref(&report_finding)) {
        if recommendations
            .iter()
            .any(|existing| existing.title.eq_ignore_ascii_case(&rec.title))
        {
            continue;
        }
        recommendations.push(AttackRecommendation {
            title: rec.title,
            description: rec.description,
            priority: rec.priority.as_str().into(),
            action: None,
        });
    }

    recommendations.push(AttackRecommendation {
        title: "Verify remediation with a re-scan".into(),
        description:
            "After applying controls, re-run this attack category against the same endpoint to confirm residual risk is reduced."
                .into(),
        priority: "medium".into(),
        action: None,
    });

    AttackRecommendationsBundle {
        overview: format!(
            "Remediation guidance for \"{}\" ({}, {}).",
            finding.title,
            category.replace('_', " "),
            finding.severity.to_ascii_lowercase()
        ),
        recommendations,
    }
}

fn finding_recommendations_fingerprint(finding: &Finding) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    finding.id.hash(&mut hasher);
    finding.title.hash(&mut hasher);
    finding.severity.to_ascii_lowercase().hash(&mut hasher);
    finding
        .category
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase()
        .hash(&mut hasher);
    finding.description.as_deref().unwrap_or("").hash(&mut hasher);
    finding.status.to_ascii_lowercase().hash(&mut hasher);
    FINDING_REMEDIATION_PROMPT_VERSION.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn load_stored_recommendations(evidence_json: Option<&str>) -> Option<StoredFindingRecommendations> {
    let raw = evidence_json?;
    let evidence: serde_json::Value = serde_json::from_str(raw).ok()?;
    let value = evidence.get(EVIDENCE_RECOMMENDATIONS_KEY)?.clone();
    let stored: StoredFindingRecommendations = serde_json::from_value(value).ok()?;
    if stored.overview.trim().is_empty() || stored.recommendations.is_empty() {
        return None;
    }
    Some(stored)
}

async fn persist_recommendations(
    repos: &promptlab_storage::Repositories,
    finding: &Finding,
    response: &FindingRecommendationsResponse,
    input_fingerprint: &str,
) -> Result<(), String> {
    let mut evidence = match finding.evidence_json.as_deref() {
        Some(raw) => serde_json::from_str::<serde_json::Value>(raw)
            .unwrap_or_else(|_| serde_json::json!({})),
        None => serde_json::json!({}),
    };
    if !evidence.is_object() {
        evidence = serde_json::json!({});
    }

    let stored = StoredFindingRecommendations {
        overview: response.overview.clone(),
        source: response.source.clone(),
        recommendations: response.recommendations.clone(),
        generated_at: response.generated_at.clone(),
        input_fingerprint: input_fingerprint.to_string(),
    };
    evidence[EVIDENCE_RECOMMENDATIONS_KEY] =
        serde_json::to_value(stored).map_err(|e| e.to_string())?;

    repos
        .findings()
        .update(
            &finding.id,
            UpdateFinding {
                evidence_json: Some(evidence),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn finding_recommendations_generate(
    state: State<'_, AppState>,
    request: FindingRecommendationsRequest,
) -> CommandResult<FindingRecommendationsResponse> {
    finding_recommendations_generate_op(state.inner(), request).await
}
