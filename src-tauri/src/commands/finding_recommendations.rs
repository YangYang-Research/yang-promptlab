//! AI-backed remediation recommendations for a single finding.
//!
//! Uses a dedicated finding-remediation system prompt (not scan Recommendations).
//! Stored in finding `evidence_json.recommendations`: reuse when `source=ai`;
//! if missing or `fallback`, retry AI on load and persist a successful result.

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
const EVIDENCE_EXCERPT_CHARS: usize = 480;

#[derive(Debug, Deserialize)]
pub struct FindingRecommendationsRequest {
    pub finding_id: String,
    /// When true, regenerate and overwrite recommendations stored on the finding.
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

    // DB is source of truth only for successful AI results.
    // Fallback (or missing) always retries AI on load unless force already requested regen.
    if !request.force {
        if let Some(stored) = load_stored_recommendations(finding.evidence_json.as_deref()) {
            if stored.source == "ai" {
                return Ok(FindingRecommendationsResponse {
                    source: stored.source,
                    overview: stored.overview,
                    recommendations: stored.recommendations,
                    generated_at: stored.generated_at,
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

    // Persist AI always. Persist fallback only when nothing better is stored (or force).
    let existing = load_stored_recommendations(finding.evidence_json.as_deref());
    let should_persist = response.source == "ai"
        || request.force
        || existing.as_ref().map(|s| s.source != "ai").unwrap_or(true);

    if should_persist {
        if let Err(err) = persist_recommendations(&repos, &finding, &response).await {
            warn!(
                finding_id = %request.finding_id,
                error = %err,
                "failed to persist finding recommendations"
            );
        }
    }

    Ok(response)
}

fn finding_remediation_input(finding: &Finding) -> FindingRemediationInput {
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
        // Metadata-only evidence (payload / confidence / verdict) — no HTTP bodies.
        evidence: slim_finding_evidence(finding.evidence_json.as_deref()),
    }
}

fn slim_finding_evidence(raw: Option<&str>) -> Option<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(raw?).ok()?;
    let obj = value.as_object()?;
    let mut slim = serde_json::Map::new();

    // Metadata-only evidence for the LLM — no HTTP response bodies.
    if let Some(payload) = string_field(obj, &["payload"]) {
        slim.insert(
            "payload".into(),
            serde_json::Value::String(truncate(&payload, EVIDENCE_EXCERPT_CHARS)),
        );
    }
    if let Some(confidence) = obj.get("confidence").cloned() {
        slim.insert("confidence".into(), confidence);
    }
    if let Some(verdict) = string_field(obj, &["verdict"]) {
        slim.insert("verdict".into(), serde_json::Value::String(verdict));
    }

    if slim.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(slim))
    }
}

fn string_field(obj: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = obj.get(*key) {
            if let Some(text) = value.as_str() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

fn truncate(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_string();
    }
    let mut out: String = value.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
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
