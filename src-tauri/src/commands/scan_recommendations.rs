//! AI-backed remediation recommendations for scan results (wizard step 6).

use aisec_report::{generate_recommendations, data::StorageFindingRow};
use aisec_storage::{Finding, FindingRepository, ScanRepository};
use aisec_target_profile::{
    build_attack_results_summary, generate_attack_recommendations_with_llm, AttackRecommendation,
    FindingSummaryInput,
};
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::warn;

use crate::error::CommandResult;
use crate::inference_host::{is_inference_ready, HostAttackRecommendLlm};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ScanRecommendationsRequest {
    pub scan_id: String,
    #[serde(default)]
    pub attack_categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttackRecommendationDto {
    pub title: String,
    pub description: String,
    pub priority: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanRecommendationsResponse {
    pub source: String,
    pub recommendations: Vec<AttackRecommendationDto>,
}

pub async fn scan_recommendations_generate_op(
    state: &AppState,
    request: ScanRecommendationsRequest,
) -> CommandResult<ScanRecommendationsResponse> {
    let scan = state
        .repositories()
        .scans()
        .get(&request.scan_id)
        .await
        .map_err(crate::error::CommandError::from)?;

    let findings = state
        .repositories()
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
            Ok(recommendations) => {
                return Ok(ScanRecommendationsResponse {
                    source: "ai".into(),
                    recommendations: recommendations.into_iter().map(Into::into).collect(),
                });
            }
            Err(err) => {
                warn!(scan_id = %request.scan_id, error = %err, "AI recommendations failed; using rule-based fallback");
            }
        }
    }

    Ok(ScanRecommendationsResponse {
        source: "fallback".into(),
        recommendations: fallback_recommendations(&findings),
    })
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

fn fallback_recommendations(findings: &[Finding]) -> Vec<AttackRecommendationDto> {
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

    generate_recommendations(&report_findings)
        .into_iter()
        .map(|rec| AttackRecommendationDto {
            title: rec.title,
            description: rec.description,
            priority: rec.priority.as_str().into(),
        })
        .collect()
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
