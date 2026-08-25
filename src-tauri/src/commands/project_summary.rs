//! AI-backed project posture summary (Project Details → Summary).

use std::sync::Arc;

use promptlab_agent::{MemoryContext, SummaryRequest, YazgDelegation, YazgSupervisor};
use promptlab_storage::{
    FindingRepository, ProjectRepository, ScanRepository, TargetRepository, UpdateProject,
};
use promptlab_target_profile::{
    ensure_failed_project_summary_action, is_retryable_scan_status, SummaryAction, SummaryBundle,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::State;
use tracing::warn;

use crate::agent_memory::SqliteAgentMemoryStore;
use crate::error::{CommandError, CommandResult};
use crate::inference_host::{is_inference_ready, YazgHostLlms};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ProjectSummaryRequest {
    pub project_id: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummaryFailedScanDto {
    pub scan_id: String,
    pub scan_name: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_name: Option<String>,
    /// Full endpoint URL for the target (preferred display label).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummaryActionDto {
    pub title: String,
    pub description: String,
    pub action: String,
    pub scan_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummaryResponse {
    pub source: String,
    pub overview: String,
    pub highlights: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed_scans: Vec<ProjectSummaryFailedScanDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<ProjectSummaryActionDto>,
    pub generated_at: String,
    pub target_count: usize,
    pub scan_count: usize,
    pub finding_count: usize,
}

#[derive(Debug, Serialize)]
struct ProjectSummaryInput {
    project_name: String,
    project_description: Option<String>,
    target_count: usize,
    scan_count: usize,
    finding_count: usize,
    severity_counts: serde_json::Map<String, serde_json::Value>,
    targets: Vec<ProjectSummaryTarget>,
    /// Failed/cancelled/stopped attack scans (full endpoint + scan_id for LLM wording).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    failed_scans: Vec<ProjectSummaryFailedScanDto>,
    recent_findings: Vec<ProjectSummaryFinding>,
}

#[derive(Debug, Serialize)]
struct ProjectSummaryTarget {
    name: String,
    target_type: String,
    url: String,
    scan_count: usize,
    /// Latest attack-scan status for this target (`none` when never scanned).
    latest_scan_status: String,
    /// Counts of attack-scan statuses on this target (e.g. completed/failed/running).
    scan_status_counts: serde_json::Map<String, serde_json::Value>,
    finding_count: usize,
    severity_counts: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct ProjectSummaryFinding {
    title: String,
    severity: String,
    category: String,
    target_name: String,
}

#[derive(Debug, Deserialize)]
struct LlmProjectSummary {
    overview: String,
    #[serde(default)]
    highlights: Vec<String>,
}

fn collect_retryable_failed_scans(
    attack_scans: &[&promptlab_storage::Scan],
    targets: &[promptlab_storage::Target],
) -> Vec<ProjectSummaryFailedScanDto> {
    let target_by_id: std::collections::HashMap<&str, &promptlab_storage::Target> = targets
        .iter()
        .map(|t| (t.id.as_str(), t))
        .collect();

    attack_scans
        .iter()
        .copied()
        .filter(|scan| is_retryable_scan_status(&scan.status))
        .map(|scan| {
            let target = scan
                .target_id
                .as_deref()
                .and_then(|tid| target_by_id.get(tid).copied());
            let target_url = target
                .map(|t| extract_url(&t.descriptor_json))
                .filter(|u| !u.trim().is_empty());
            ProjectSummaryFailedScanDto {
                scan_id: scan.id.clone(),
                scan_name: scan.name.clone(),
                status: scan.status.clone(),
                target_id: scan.target_id.clone(),
                target_name: target.map(|t| t.name.clone()),
                target_url,
            }
        })
        .collect()
}

fn attach_retry_actions(
    bundle: SummaryBundle,
    failed: &[ProjectSummaryFailedScanDto],
) -> SummaryBundle {
    ensure_failed_project_summary_action(!failed.is_empty(), bundle)
}

/// One Retry Scan CTA per failed/cancelled/stopped attack scan.
fn actions_to_dto(failed: &[ProjectSummaryFailedScanDto]) -> Vec<ProjectSummaryActionDto> {
    failed
        .iter()
        .map(|scan| {
            let endpoint = scan
                .target_url
                .as_deref()
                .filter(|u| !u.trim().is_empty())
                .or(scan.target_name.as_deref())
                .unwrap_or("target");
            ProjectSummaryActionDto {
                title: "Retry Scan".into(),
                description: format!(
                    "Open the scan wizard to Retry Scan for {endpoint} ({})",
                    scan.scan_id
                ),
                action: "retry_scan".into(),
                scan_id: scan.scan_id.clone(),
                target_id: scan.target_id.clone(),
            }
        })
        .collect()
}

pub async fn project_summary_generate_op(
    state: &AppState,
    request: ProjectSummaryRequest,
) -> CommandResult<ProjectSummaryResponse> {
    let project_id = request.project_id.trim();
    if project_id.is_empty() {
        return Err(CommandError::invalid_input("project id must not be empty"));
    }

    let repos = state.repositories();
    let project = repos
        .projects()
        .get(project_id)
        .await
        .map_err(CommandError::from)?;

    let targets = repos
        .targets()
        .list_by_project(project_id)
        .await
        .map_err(CommandError::from)?;

    if targets.is_empty() {
        return Err(CommandError::invalid_input(
            "project summary requires at least one target",
        ));
    }

    let scans = repos
        .scans()
        .list_by_project(project_id)
        .await
        .map_err(CommandError::from)?;

    let attack_scans: Vec<_> = scans
        .iter()
        .filter(|scan| {
            scan.name.starts_with("Scan (") || scan.name.starts_with("Agent Scan (")
        })
        .collect();
    let target_name_by_id: std::collections::HashMap<&str, &str> = targets
        .iter()
        .map(|t| (t.id.as_str(), t.name.as_str()))
        .collect();
    let failed_scans = collect_retryable_failed_scans(&attack_scans, &targets);

    let findings = repos
        .findings()
        .list_by_project(project_id)
        .await
        .map_err(CommandError::from)?;

    // Reuse only a successful AI result from DB. Fallback results retry AI on load.
    // Re-attach retry actions from live scan state so failed scans get current CTAs.
    if !request.force {
        if let Some(mut cached) = load_stored_summary(project.summary_json.as_deref()) {
            if cached.source == "ai" {
                let ensured = attach_retry_actions(
                    SummaryBundle {
                        overview: cached.overview.clone(),
                        highlights: cached.highlights.clone(),
                        actions: cached
                            .actions
                            .iter()
                            .map(|a| SummaryAction {
                                title: a.title.clone(),
                                description: a.description.clone(),
                                action: a.action.clone(),
                            })
                            .collect(),
                    },
                    &failed_scans,
                );
                cached.overview = ensured.overview;
                cached.highlights = ensured.highlights;
                cached.failed_scans = failed_scans.clone();
                cached.actions = actions_to_dto(&failed_scans);
                return Ok(cached);
            }
        }
    }

    let mut severity_counts = serde_json::Map::new();
    for finding in &findings {
        let key = finding.severity.to_ascii_lowercase();
        let entry = severity_counts.entry(key).or_insert(json!(0));
        if let Some(n) = entry.as_u64() {
            *entry = json!(n + 1);
        }
    }

    let scan_target_by_id: std::collections::HashMap<&str, Option<&str>> = scans
        .iter()
        .map(|scan| (scan.id.as_str(), scan.target_id.as_deref()))
        .collect();

    let mut findings_by_target: std::collections::HashMap<&str, Vec<&promptlab_storage::Finding>> =
        std::collections::HashMap::new();
    for finding in &findings {
        let target_id = finding
            .target_id
            .as_deref()
            .or_else(|| {
                scan_target_by_id
                    .get(finding.scan_id.as_str())
                    .and_then(|id| *id)
            });
        if let Some(tid) = target_id {
            findings_by_target.entry(tid).or_default().push(finding);
        }
    }

    // Attack scans per target, newest first (list_by_project is created_at DESC).
    let mut scans_by_target: std::collections::HashMap<&str, Vec<&promptlab_storage::Scan>> =
        std::collections::HashMap::new();
    for scan in &attack_scans {
        if let Some(tid) = scan.target_id.as_deref() {
            scans_by_target.entry(tid).or_default().push(scan);
        }
    }

    let input = ProjectSummaryInput {
        project_name: project.name.clone(),
        project_description: project.description.clone(),
        target_count: targets.len(),
        scan_count: attack_scans.len(),
        finding_count: findings.len(),
        severity_counts,
        targets: targets
            .iter()
            .map(|t| {
                let target_findings = findings_by_target.get(t.id.as_str());
                let mut target_severity = serde_json::Map::new();
                if let Some(list) = target_findings {
                    for finding in list {
                        let key = finding.severity.to_ascii_lowercase();
                        let entry = target_severity.entry(key).or_insert(json!(0));
                        if let Some(n) = entry.as_u64() {
                            *entry = json!(n + 1);
                        }
                    }
                }
                let target_scans = scans_by_target.get(t.id.as_str());
                let mut scan_status_counts = serde_json::Map::new();
                if let Some(list) = target_scans {
                    for scan in list {
                        let key = scan.status.to_ascii_lowercase();
                        let entry = scan_status_counts.entry(key).or_insert(json!(0));
                        if let Some(n) = entry.as_u64() {
                            *entry = json!(n + 1);
                        }
                    }
                }
                let latest_scan_status = target_scans
                    .and_then(|list| list.first())
                    .map(|scan| scan.status.to_ascii_lowercase())
                    .unwrap_or_else(|| "none".into());
                ProjectSummaryTarget {
                    name: t.name.clone(),
                    target_type: t.target_type.clone(),
                    url: extract_url(&t.descriptor_json),
                    scan_count: target_scans.map(|list| list.len()).unwrap_or(0),
                    latest_scan_status,
                    scan_status_counts,
                    finding_count: target_findings.map(|list| list.len()).unwrap_or(0),
                    severity_counts: target_severity,
                }
            })
            .collect(),
        failed_scans: failed_scans.clone(),
        recent_findings: findings
            .iter()
            .take(20)
            .map(|f| {
                let target_name = f
                    .target_id
                    .as_deref()
                    .or_else(|| {
                        scan_target_by_id
                            .get(f.scan_id.as_str())
                            .and_then(|id| *id)
                    })
                    .and_then(|tid| target_name_by_id.get(tid).copied())
                    .unwrap_or("unknown")
                    .to_string();
                ProjectSummaryFinding {
                    title: f.title.clone(),
                    severity: f.severity.clone(),
                    category: f.category.clone().unwrap_or_else(|| "unknown".into()),
                    target_name,
                }
            })
            .collect(),
    };

    let generated_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".into());

    let llm_bundle = {
        let inference = state.inference_manager().lock().await;
        if is_inference_ready(&inference) {
            drop(inference);
            match serde_json::to_string(&input) {
                Ok(input_json) => {
                    let hosts = YazgHostLlms::from_app(
                        state.data_dir().to_path_buf(),
                        state.inference_manager().clone(),
                        state.model_manager().clone(),
                        state.model_provider().clone(),
                        state.runtime_manager().clone(),
                    );
                    let llms = hosts.into_yazg_llms();
                    let summary_request = SummaryRequest::Project {
                        project_name: input.project_name.clone(),
                        input_json,
                    };
                    let memory: Arc<dyn promptlab_agent::AgentMemoryStore> =
                        Arc::new(SqliteAgentMemoryStore::new(state.repositories()));
                    let memory_ctx =
                        MemoryContext::new(format!("project-summary:{project_id}"))
                            .with_project(Some(project_id.to_string()));
                    match YazgSupervisor::react_summarize(
                        &summary_request,
                        &llms,
                        Some(memory),
                        memory_ctx,
                    )
                    .await
                    {
                        Ok(YazgDelegation::Summarized { outcome, .. }) => {
                            Some(LlmProjectSummary {
                                overview: outcome.bundle.overview,
                                highlights: outcome.bundle.highlights,
                            })
                        }
                        Ok(_) => {
                            warn!(
                                project_id = %project_id,
                                "Yazg ReAct finished without SummaryAgent; using rule-based fallback"
                            );
                            None
                        }
                        Err(err) => {
                            warn!(
                                project_id = %project_id,
                                error = %err,
                                "AI project summary failed; using rule-based fallback"
                            );
                            None
                        }
                    }
                }
                Err(err) => {
                    warn!(
                        project_id = %project_id,
                        error = %err,
                        "failed to serialize project summary input; using rule-based fallback"
                    );
                    None
                }
            }
        } else {
            None
        }
    };

    let (source, bundle) = match llm_bundle {
        Some(bundle) => (
            "ai".into(),
            SummaryBundle {
                overview: bundle.overview,
                highlights: bundle.highlights,
                actions: Vec::new(),
            },
        ),
        None => {
            let fallback = fallback_summary(&input);
            (
                "fallback".into(),
                SummaryBundle {
                    overview: fallback.overview,
                    highlights: fallback.highlights,
                    actions: Vec::new(),
                },
            )
        }
    };

    let bundle = attach_retry_actions(bundle, &failed_scans);

    let response = ProjectSummaryResponse {
        source,
        overview: bundle.overview,
        highlights: bundle.highlights,
        failed_scans: failed_scans.clone(),
        actions: actions_to_dto(&failed_scans),
        generated_at,
        target_count: input.target_count,
        scan_count: input.scan_count,
        finding_count: input.finding_count,
    };

    let existing = load_stored_summary(project.summary_json.as_deref());
    let should_persist = response.source == "ai"
        || request.force
        || existing.as_ref().map(|s| s.source != "ai").unwrap_or(true);
    if should_persist {
        if let Err(err) = persist_summary(&repos, project_id, &response).await {
            warn!(
                project_id = %project_id,
                error = %err,
                "failed to persist project summary"
            );
        }
    }

    Ok(response)
}

fn fallback_summary(input: &ProjectSummaryInput) -> LlmProjectSummary {
    let unscanned = input.targets.iter().filter(|t| t.scan_count == 0).count();
    let never_scanned = input.scan_count == 0 || unscanned == input.target_count;
    let retryable = input.targets.iter().any(|t| {
        is_retryable_scan_status(&t.latest_scan_status)
            || t.scan_status_counts.keys().any(|k| is_retryable_scan_status(k))
    });

    let overview = if never_scanned {
        format!(
            "{} has {} target{} but no attack scans yet, so risk posture is unknown. Start authorized baseline scans on the highest-value endpoints to establish coverage before remediating anything.",
            input.project_name,
            input.target_count,
            if input.target_count == 1 { "" } else { "s" },
        )
    } else if input.finding_count == 0 {
        format!(
            "{} currently has {} target{} and {} attack scan{} with no recorded findings. Maintain continuous authorized testing and baseline hardening across the project scope.",
            input.project_name,
            input.target_count,
            if input.target_count == 1 { "" } else { "s" },
            input.scan_count,
            if input.scan_count == 1 { "" } else { "s" },
        )
    } else {
        format!(
            "{} covers {} target{} with {} attack scan{} and {} finding{}. Prioritize remediating the highest-severity issues and re-test affected targets after fixes land.",
            input.project_name,
            input.target_count,
            if input.target_count == 1 { "" } else { "s" },
            input.scan_count,
            if input.scan_count == 1 { "" } else { "s" },
            input.finding_count,
            if input.finding_count == 1 { "" } else { "s" },
        )
    };

    let mut highlights = Vec::new();
    if !input.failed_scans.is_empty() {
        let parts: Vec<String> = input
            .failed_scans
            .iter()
            .map(|f| {
                let endpoint = f
                    .target_url
                    .as_deref()
                    .filter(|u| !u.trim().is_empty())
                    .or(f.target_name.as_deref())
                    .unwrap_or("unknown endpoint");
                format!("{endpoint} (scan {})", f.scan_id)
            })
            .collect();
        highlights.push(format!(
            "Retry Scan for failed assessment{} on {} to restore coverage confidence",
            if input.failed_scans.len() == 1 { "" } else { "s" },
            parts.join("; "),
        ));
    }
    highlights.push(format!(
        "Inventory: {} targets · {} scans · {} findings",
        input.target_count, input.scan_count, input.finding_count
    ));

    if never_scanned {
        highlights.push(
            "Launch an authorized attack scan on the most critical target first to establish a security baseline"
                .into(),
        );
        highlights.push(
            "Verify authentication and endpoint reachability before running adversarial tests".into(),
        );
        if input.target_count > 1 {
            highlights.push(format!(
                "Schedule follow-up scans across the remaining {} target{} to close coverage gaps",
                input.target_count.saturating_sub(1),
                if input.target_count == 2 { "" } else { "s" },
            ));
        }
        highlights.push(
            "After the first scan completes, use findings severity to prioritize remediation and re-tests"
                .into(),
        );
        return LlmProjectSummary {
            overview,
            highlights,
        };
    }

    if let Some(crit) = input.severity_counts.get("critical").and_then(|v| v.as_u64()) {
        if crit > 0 {
            highlights.push(format!("{crit} critical finding(s) require immediate remediation"));
        }
    }
    if let Some(high) = input.severity_counts.get("high").and_then(|v| v.as_u64()) {
        if high > 0 {
            highlights.push(format!("{high} high-severity finding(s) should be scheduled next"));
        }
    }
    if unscanned > 0 {
        highlights.push(format!(
            "{unscanned} target(s) still have no attack scans — expand coverage next"
        ));
    } else if input.finding_count == 0 {
        highlights.push(
            "No confirmed vulnerabilities yet — keep periodic adversarial scans running".into(),
        );
    } else if let Some(hottest) = input
        .targets
        .iter()
        .max_by_key(|t| t.finding_count)
        .filter(|t| t.finding_count > 0)
    {
        highlights.push(format!(
            "Hottest target: {} ({} finding{})",
            hottest.name,
            hottest.finding_count,
            if hottest.finding_count == 1 { "" } else { "s" }
        ));
    } else if let Some(first) = input.recent_findings.first() {
        highlights.push(format!(
            "Latest notable finding: {} ({}) on {}",
            first.title, first.severity, first.target_name
        ));
    }

    let failed_targets: Vec<&str> = input
        .targets
        .iter()
        .filter(|t| {
            is_retryable_scan_status(&t.latest_scan_status)
                || t.scan_status_counts
                    .keys()
                    .any(|k| is_retryable_scan_status(k))
        })
        .map(|t| t.name.as_str())
        .collect();
    if !failed_targets.is_empty() && !retryable {
        highlights.push(format!(
            "Failed scan(s) on {} — investigate and re-run before trusting coverage",
            failed_targets.join(", ")
        ));
    }

    highlights.push("Re-run attack scans after remediations to validate residual risk".into());

    LlmProjectSummary {
        overview,
        highlights,
    }
}

async fn persist_summary(
    repos: &promptlab_storage::Repositories,
    project_id: &str,
    response: &ProjectSummaryResponse,
) -> Result<(), String> {
    let stored = serde_json::to_string(response).map_err(|e| e.to_string())?;
    repos
        .projects()
        .update(
            project_id,
            UpdateProject {
                summary_json: Some(stored),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn load_stored_summary(raw: Option<&str>) -> Option<ProjectSummaryResponse> {
    let raw = raw?;
    let stored: ProjectSummaryResponse = serde_json::from_str(raw).ok()?;
    if stored.overview.trim().is_empty() {
        return None;
    }
    Some(stored)
}

fn extract_url(descriptor_json: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(descriptor_json).unwrap_or(json!({}));
    value
        .get("url")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("base_url").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string()
}

#[tauri::command]
pub async fn project_summary_generate(
    state: State<'_, AppState>,
    request: ProjectSummaryRequest,
) -> CommandResult<ProjectSummaryResponse> {
    project_summary_generate_op(state.inner(), request).await
}
