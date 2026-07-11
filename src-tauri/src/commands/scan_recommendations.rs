//! AI-backed remediation recommendations for scan results (wizard step 6).

use aisec_report::{generate_recommendations, data::StorageFindingRow};
use aisec_storage::{Finding, FindingRepository, ScanRepository, UpdateScan};
use aisec_target_profile::{
    build_attack_results_summary, generate_attack_recommendations_with_llm, AttackRecommendation,
    AttackRecommendationsBundle, FindingSummaryInput,
};
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::warn;

use crate::error::CommandResult;
use crate::inference_host::{is_inference_ready, HostAttackRecommendLlm};
use crate::state::AppState;

const PLAYBOOK_RECOMMENDATIONS_KEY: &str = "recommendations";

#[derive(Debug, Deserialize)]
pub struct ScanRecommendationsRequest {
    pub scan_id: String,
    #[serde(default)]
    pub attack_categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackRecommendationDto {
    pub title: String,
    pub description: String,
    pub priority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredRecommendations {
    overview: String,
    source: String,
    recommendations: Vec<AttackRecommendationDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanRecommendationsResponse {
    pub source: String,
    pub overview: String,
    pub recommendations: Vec<AttackRecommendationDto>,
}

pub async fn scan_recommendations_generate_op(
    state: &AppState,
    request: ScanRecommendationsRequest,
) -> CommandResult<ScanRecommendationsResponse> {
    let repos = state.repositories();
    let scan = repos
        .scans()
        .get(&request.scan_id)
        .await
        .map_err(crate::error::CommandError::from)?;

    if let Some(cached) = load_stored_recommendations(scan.playbook_json.as_deref()) {
        return Ok(ScanRecommendationsResponse {
            source: cached.source,
            overview: cached.overview,
            recommendations: cached.recommendations,
        });
    }

    let findings = repos
        .findings()
        .list_by_scan(&request.scan_id)
        .await
        .map_err(crate::error::CommandError::from)?;

    let summary_inputs: Vec<FindingSummaryInput> =
        findings.iter().map(finding_to_summary).collect();
    let summary = build_attack_results_summary(
        &scan.status,
        &request.attack_categories,
        &summary_inputs,
    );

    let bundle = {
        let inference = state.inference_manager().lock().await;
        if is_inference_ready(&inference) {
            drop(inference);
            let llm = HostAttackRecommendLlm::new(
                state.data_dir().to_path_buf(),
                state.inference_manager().clone(),
                state.model_manager().clone(),
                state.model_provider().clone(),
                state.runtime_manager().clone(),
            );
            match generate_attack_recommendations_with_llm(&summary, &llm).await {
                Ok(recommendations) => Some(("ai", recommendations)),
                Err(err) => {
                    warn!(
                        scan_id = %request.scan_id,
                        error = %err,
                        "AI recommendations failed; using rule-based fallback"
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
        None => ("fallback", fallback_recommendations_bundle(&findings)),
    };

    let response = ScanRecommendationsResponse {
        source: source.into(),
        overview: bundle.overview,
        recommendations: bundle
            .recommendations
            .into_iter()
            .map(Into::into)
            .collect(),
    };

    if let Err(err) = persist_recommendations(&repos, &request.scan_id, &response).await {
        warn!(
            scan_id = %request.scan_id,
            error = %err,
            "failed to persist scan recommendations"
        );
    }

    Ok(response)
}

async fn persist_recommendations(
    repos: &aisec_storage::Repositories,
    scan_id: &str,
    response: &ScanRecommendationsResponse,
) -> Result<(), String> {
    let scan = repos
        .scans()
        .get(scan_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut playbook = match scan.playbook_json.as_deref() {
        Some(raw) => serde_json::from_str::<serde_json::Value>(raw)
            .unwrap_or_else(|_| serde_json::json!({})),
        None => serde_json::json!({}),
    };
    if !playbook.is_object() {
        playbook = serde_json::json!({});
    }

    let stored = StoredRecommendations {
        overview: response.overview.clone(),
        source: response.source.clone(),
        recommendations: response.recommendations.clone(),
    };
    playbook[PLAYBOOK_RECOMMENDATIONS_KEY] =
        serde_json::to_value(stored).map_err(|e| e.to_string())?;

    repos
        .scans()
        .update(
            scan_id,
            UpdateScan {
                playbook_json: Some(playbook),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn load_stored_recommendations(playbook_json: Option<&str>) -> Option<StoredRecommendations> {
    let raw = playbook_json?;
    let playbook: serde_json::Value = serde_json::from_str(raw).ok()?;
    let value = playbook.get(PLAYBOOK_RECOMMENDATIONS_KEY)?.clone();
    let stored: StoredRecommendations = serde_json::from_value(value).ok()?;
    if stored.overview.trim().is_empty() || stored.recommendations.is_empty() {
        return None;
    }
    Some(stored)
}

fn finding_to_summary(finding: &Finding) -> FindingSummaryInput {
    FindingSummaryInput {
        title: finding.title.clone(),
        severity: finding.severity.clone(),
        category: finding
            .category
            .clone()
            .unwrap_or_else(|| "unknown".into()),
        description: finding.description.clone(),
    }
}

fn fallback_recommendations_bundle(findings: &[Finding]) -> AttackRecommendationsBundle {
    let overview = if findings.is_empty() {
        "No vulnerabilities were confirmed in this scan; maintain baseline hardening and continuous testing for the scoped attack surface.".into()
    } else {
        format!(
            "This scan identified {} finding{}; prioritize remediating the highest-severity issues first.",
            findings.len(),
            if findings.len() == 1 { "" } else { "s" }
        )
    };

    let report_findings: Vec<_> = findings
        .iter()
        .map(|f| {
            StorageFindingRow {
                id: f.id.clone(),
                title: f.title.clone(),
                severity: f.severity.clone(),
                category: f.category.clone(),
                description: f.description.clone(),
                evidence_json: f.evidence_json.clone(),
                status: f.status.clone(),
            }
            .into_report_finding()
        })
        .collect();

    let recommendations: Vec<AttackRecommendation> = generate_recommendations(&report_findings)
        .into_iter()
        .map(|rec| AttackRecommendation {
            title: rec.title,
            description: rec.description,
            priority: rec.priority.as_str().into(),
        })
        .collect();

    let recommendations = if recommendations.is_empty() {
        vec![AttackRecommendation {
            title: "Schedule continuous authorized testing".into(),
            description: "Re-run adversarial assessments periodically and keep input/output guardrails enabled even when a single scan is clean.".into(),
            priority: "info".into(),
        }]
    } else {
        recommendations
    };

    AttackRecommendationsBundle {
        overview,
        recommendations,
    }
}

impl From<AttackRecommendation> for AttackRecommendationDto {
    fn from(value: AttackRecommendation) -> Self {
        Self {
            title: value.title,
            description: value.description,
            priority: value.priority,
        }
    }
}

#[tauri::command]
pub async fn scan_recommendations_generate(
    state: State<'_, AppState>,
    request: ScanRecommendationsRequest,
) -> CommandResult<ScanRecommendationsResponse> {
    scan_recommendations_generate_op(state.inner(), request).await
}
