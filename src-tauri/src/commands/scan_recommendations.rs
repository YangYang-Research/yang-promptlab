//! AI-backed remediation recommendations for scan results (wizard step 6).

use std::sync::Arc;

use promptlab_agent::{MemoryContext, YazgDelegation, YazgSupervisor};
use promptlab_report::{generate_recommendations, data::StorageFindingRow};
use promptlab_storage::{Finding, FindingRepository, ScanRepository, TargetRepository, UpdateScan};
use promptlab_target_profile::{
    build_attack_results_summary, ensure_failed_scan_action_recommendation,
    AttackRecommendation, AttackRecommendationsBundle, FindingSummaryInput,
};
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::warn;

use crate::agent_memory::SqliteAgentMemoryStore;
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
    /// When true, regenerate and overwrite any cached recommendations in the scan playbook.
    #[serde(default)]
    pub force: bool,
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
    #[serde(default)]
    generated_at: String,
    /// Hash of scan status + findings + categories; invalidates stale cache.
    #[serde(default)]
    input_fingerprint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanRecommendationsResponse {
    pub source: String,
    pub overview: String,
    pub recommendations: Vec<AttackRecommendationDto>,
    pub generated_at: String,
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

    let findings = repos
        .findings()
        .list_by_scan(&request.scan_id)
        .await
        .map_err(crate::error::CommandError::from)?;

    let input_fingerprint =
        scan_recommendations_fingerprint(&scan, &request.attack_categories, &findings);

    if !request.force {
        if let Some(mut cached) = load_stored_recommendations(scan.playbook_json.as_deref()) {
            if !cached.input_fingerprint.is_empty()
                && cached.input_fingerprint == input_fingerprint
            {
                if cached.generated_at.trim().is_empty() {
                    cached.generated_at = scan
                        .updated_at
                        .format(&time::format_description::well_known::Rfc3339)
                        .unwrap_or_else(|_| scan.updated_at.to_string());
                    let backfill = ScanRecommendationsResponse {
                        source: cached.source.clone(),
                        overview: cached.overview.clone(),
                        recommendations: cached.recommendations.clone(),
                        generated_at: cached.generated_at.clone(),
                    };
                    if let Err(err) =
                        persist_recommendations(&repos, &request.scan_id, &backfill, &input_fingerprint)
                            .await
                    {
                        warn!(
                            scan_id = %request.scan_id,
                            error = %err,
                            "failed to backfill recommendations generated_at"
                        );
                    }
                }
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
                    generated_at: cached.generated_at,
                });
            }
        }
    }

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
            let llms = hosts.into_rig_llms();
            let memory: Arc<dyn promptlab_agent::AgentMemoryStore> =
                Arc::new(SqliteAgentMemoryStore::new(state.repositories()));
            let memory_ctx = MemoryContext::new(format!("scan-recommend:{}", request.scan_id))
                .with_project(Some(scan.project_id.clone()))
                .with_target(scan.target_id.clone())
                .with_scan(Some(request.scan_id.clone()));
            match YazgSupervisor::react_recommend(&summary, &llms, Some(memory), memory_ctx)
                .await
            {
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

    let generated_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".into());

    let response = ScanRecommendationsResponse {
        source: source.into(),
        overview: bundle.overview,
        recommendations: bundle
            .recommendations
            .into_iter()
            .map(Into::into)
            .collect(),
        generated_at,
    };

    if let Err(err) =
        persist_recommendations(&repos, &request.scan_id, &response, &input_fingerprint).await
    {
        warn!(
            scan_id = %request.scan_id,
            error = %err,
            "failed to persist scan recommendations"
        );
    }

    Ok(response)
}

async fn persist_recommendations(
    repos: &promptlab_storage::Repositories,
    scan_id: &str,
    response: &ScanRecommendationsResponse,
    input_fingerprint: &str,
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
        generated_at: response.generated_at.clone(),
        input_fingerprint: input_fingerprint.to_string(),
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

fn scan_recommendations_fingerprint(
    scan: &promptlab_storage::Scan,
    attack_categories: &[String],
    findings: &[Finding],
) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    scan.id.hash(&mut hasher);
    scan.status.to_ascii_lowercase().hash(&mut hasher);
    scan.name.hash(&mut hasher);
    scan.target_id.as_deref().unwrap_or("").hash(&mut hasher);

    let mut categories: Vec<&str> = attack_categories.iter().map(String::as_str).collect();
    categories.sort_unstable();
    for category in categories {
        category.hash(&mut hasher);
    }

    let mut finding_keys: Vec<_> = findings
        .iter()
        .map(|f| {
            (
                f.id.as_str(),
                f.severity.to_ascii_lowercase(),
                f.category.as_deref().unwrap_or(""),
                f.title.as_str(),
            )
        })
        .collect();
    finding_keys.sort_unstable();
    for (id, severity, category, title) in finding_keys {
        id.hash(&mut hasher);
        severity.hash(&mut hasher);
        category.hash(&mut hasher);
        title.hash(&mut hasher);
    }

    format!("{:016x}", hasher.finish())
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
