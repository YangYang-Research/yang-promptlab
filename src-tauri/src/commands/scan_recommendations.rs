//! AI-backed remediation recommendations for scan results (wizard step 6).

use aisec_agent::{YazgDelegation, YazgSupervisor};
use aisec_report::{generate_recommendations, data::StorageFindingRow};
use aisec_storage::{Finding, FindingRepository, ScanRepository, TargetRepository, UpdateScan};
use aisec_target_profile::{
    build_attack_results_summary, ensure_failed_scan_action_recommendation,
    AttackRecommendation, AttackRecommendationsBundle, FindingSummaryInput,
};
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::warn;

use crate::error::CommandResult;
use crate::inference_host::{is_inference_ready, YazgHostLlms};
use crate::session_auth::seed_url_from_descriptor;
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
    /// Optional UI action: `retry_scan` | `start_attack`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
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
        let mut cache_summary = build_attack_results_summary(&scan.status, &[], &[]);
        cache_summary.scan_name = Some(scan.name.clone());
        let ensured = ensure_failed_scan_action_recommendation(
            &cache_summary,
            AttackRecommendationsBundle {
                overview: cached.overview.clone(),
                recommendations: cached
                    .recommendations
                    .iter()
                    .map(|r| AttackRecommendation {
                        title: r.title.clone(),
                        description: r.description.clone(),
                        priority: r.priority.clone(),
                        action: r.action.clone(),
                    })
                    .collect(),
            },
        );
        return Ok(ScanRecommendationsResponse {
            source: cached.source,
            overview: ensured.overview,
            recommendations: ensured.recommendations.into_iter().map(Into::into).collect(),
        });
    }

    let findings = repos
        .findings()
        .list_by_scan(&request.scan_id)
        .await
        .map_err(crate::error::CommandError::from)?;

    let summary_inputs: Vec<FindingSummaryInput> =
        findings.iter().map(finding_to_summary).collect();
    let mut summary = build_attack_results_summary(
        &scan.status,
        &request.attack_categories,
        &summary_inputs,
    );
    summary.scan_name = Some(scan.name.clone());

    if let Some(target_id) = scan.target_id.as_deref() {
        if let Ok(target) = repos.targets().get(target_id).await {
            summary.target_name = Some(target.name);
            summary.target_url = seed_url_from_descriptor(&target.descriptor_json);
        }

        if let Ok(project_scans) = repos.scans().list_by_project(&scan.project_id).await {
            for sibling in project_scans.iter().filter(|s| {
                s.target_id.as_deref() == Some(target_id)
                    && (s.name.starts_with("Scan (") || s.name.starts_with("Agent Scan ("))
            }) {
                *summary
                    .target_scan_status_counts
                    .entry(sibling.status.to_ascii_lowercase())
                    .or_insert(0) += 1;
            }
        }
    }

    let bundle = {
        let inference = state.inference_manager().lock().await;
        if is_inference_ready(&inference) {
            drop(inference);
            let hosts = YazgHostLlms::from_app(
                state.data_dir().to_path_buf(),
                state.inference_manager().clone(),
                state.model_manager().clone(),
                state.model_provider().clone(),
                state.runtime_manager().clone(),
            );
            let llms = hosts.react_llms();
            match YazgSupervisor::react_recommend(&summary, &llms).await {
                Ok(YazgDelegation::Recommended { outcome, .. }) => {
                    Some(("ai", outcome.bundle))
                }
                Ok(other) => {
                    warn!(
                        scan_id = %request.scan_id,
                        "Yazg ReAct finished without RecommendAgent; using rule-based fallback"
                    );
                    let _ = other;
                    None
                }
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
        None => (
            "fallback",
            ensure_failed_scan_action_recommendation(
                &summary,
                fallback_recommendations_bundle(&findings),
            ),
        ),
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
            action: None,
        })
        .collect();

    let recommendations = if recommendations.is_empty() {
        vec![AttackRecommendation {
            title: "Schedule continuous authorized testing".into(),
            description: "Re-run adversarial assessments periodically and keep input/output guardrails enabled even when a single scan is clean.".into(),
            priority: "info".into(),
            action: None,
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
            action: value.action,
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
