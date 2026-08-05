//! Yazg supervisor chat IPC (ReAct).

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use promptlab_agent::{
    AgentEvent, CreateProjectTools, CreatedProject, HiltPendingAction, MemoryContext, SupervisorIntent,
    ToolResult, WorkspaceFindingSummary, WorkspaceInventory, WorkspaceProjectSummary, WorkspaceReportSummary,
    WorkspaceScanSummary, WorkspaceTargetSummary, WorkspaceTools, WorkspaceTotals, YazgDelegation,
    YazgSupervisor, YazgTurn, FindingDetail, FindingList, ProjectDetail, ReportDetail, ReportList,
    ScanDetail, ScanList, TargetDetail, TargetList, DEFAULT_FINDINGS_LIMIT, MAX_FINDINGS_LIMIT,
    MAX_REPORT_PREVIEW_CHARS, clamp_findings_limit, is_mutating_tool,
};
use promptlab_storage::{
    CreateProject, FindingRepository, ProjectRepository, ReportRepository, ScanRepository,
    TargetRepository,
};
use promptlab_target_profile::TargetProfile;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;
use tokio::sync::Mutex as TokioMutex;

use crate::agent_memory::SqliteAgentMemoryStore;
use crate::error::{CommandError, CommandResult};
use crate::inference_host::{gateway_complete_as, is_inference_ready, YazgHostLlms};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YazgChatRequest {
    pub message: String,
    #[serde(default)]
    pub target_id: Option<String>,
    /// Stable chat-thread session id for STM continuity within one conversation.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Soft hint only — Yazg ReAct still chooses the action.
    /// `auto` | `chat` | `analyze_endpoint` | `verify` | `attack_plan` | `plan` |
    /// `generate_prompt` | `recommend` | `summary` | `list_workspace`
    #[serde(default)]
    pub intent: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YazgCreatedProjectDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YazgHiltPendingActionDto {
    pub id: String,
    pub tool: String,
    pub kind: String,
    pub args: Value,
    pub summary: String,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YazgChatResponse {
    pub reply: String,
    pub intent: String,
    /// Actual tool that produced the reply (`intent` is a coarse category).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    pub events: Vec<AgentEventDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<Value>,
    pub verified: Option<bool>,
    pub plan_summary: Option<String>,
    pub created_project: Option<YazgCreatedProjectDto>,
    /// Mutating tool awaiting Approve / Deny in the chat UI (HILT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_action: Option<YazgHiltPendingActionDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEventDto {
    pub agent: String,
    pub kind: String,
    pub message: String,
}

struct HostCreateProjectTools {
    repos: promptlab_storage::Repositories,
}

#[async_trait]
impl CreateProjectTools for HostCreateProjectTools {
    async fn create_project(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<CreatedProject, String> {
        let project = self
            .repos
            .projects()
            .create(CreateProject {
                name: name.to_string(),
                description: description.map(str::to_string),
            })
            .await
            .map_err(|err| err.to_string())?;
        Ok(CreatedProject {
            id: project.id,
            name: project.name,
            description: project.description,
        })
    }
}

struct HostWorkspaceTools {
    repos: promptlab_storage::Repositories,
}

impl HostWorkspaceTools {
    async fn resolve_project(
        &self,
        project: &str,
    ) -> Result<promptlab_storage::Project, String> {
        let key = project.trim();
        if key.is_empty() {
            return Err("project id or name is required".into());
        }
        let projects = self
            .repos
            .projects()
            .list()
            .await
            .map_err(|err| err.to_string())?;
        if let Some(p) = projects.iter().find(|p| p.id == key) {
            return Ok(p.clone());
        }
        let lower = key.to_lowercase();
        let matches: Vec<_> = projects
            .into_iter()
            .filter(|p| p.name.to_lowercase() == lower)
            .collect();
        matches
            .into_iter()
            .next()
            .ok_or_else(|| format!("project not found: {key}"))
    }

    async fn project_counts(
        &self,
        project_id: &str,
    ) -> Result<(usize, usize, usize), String> {
        let targets = self
            .repos
            .targets()
            .list_by_project(project_id)
            .await
            .map_err(|e| e.to_string())?
            .len();
        let scans = self
            .repos
            .scans()
            .list_by_project(project_id)
            .await
            .map_err(|e| e.to_string())?
            .len();
        let findings = self
            .repos
            .findings()
            .list_by_project(project_id)
            .await
            .map_err(|e| e.to_string())?
            .len();
        Ok((targets, scans, findings))
    }
}

#[async_trait]
impl WorkspaceTools for HostWorkspaceTools {
    async fn list_workspace(&self) -> Result<WorkspaceInventory, String> {
        let projects = self
            .repos
            .projects()
            .list()
            .await
            .map_err(|err| err.to_string())?;

        let mut total_targets = 0usize;
        let mut total_scans = 0usize;
        let mut total_findings = 0usize;
        let mut summaries = Vec::with_capacity(projects.len());

        for project in projects {
            let (targets_count, scans_count, findings_count) =
                self.project_counts(&project.id).await?;
            total_targets += targets_count;
            total_scans += scans_count;
            total_findings += findings_count;
            summaries.push(WorkspaceProjectSummary {
                id: project.id,
                name: project.name,
                description: project.description,
                findings_count,
                targets_count,
                scans_count,
            });
        }

        Ok(WorkspaceInventory {
            totals: WorkspaceTotals {
                projects: summaries.len(),
                targets: total_targets,
                scans: total_scans,
                findings: total_findings,
                findings_truncated: 0,
            },
            projects: summaries,
        })
    }

    async fn project_detail(&self, project: &str) -> Result<ProjectDetail, String> {
        let project = self.resolve_project(project).await?;
        let targets = self
            .repos
            .targets()
            .list_by_project(&project.id)
            .await
            .map_err(|e| e.to_string())?;
        let scans = self
            .repos
            .scans()
            .list_by_project(&project.id)
            .await
            .map_err(|e| e.to_string())?;
        let findings_count = self
            .repos
            .findings()
            .list_by_project(&project.id)
            .await
            .map_err(|e| e.to_string())?
            .len();

        Ok(ProjectDetail {
            project: WorkspaceProjectSummary {
                id: project.id.clone(),
                name: project.name,
                description: project.description,
                findings_count,
                targets_count: targets.len(),
                scans_count: scans.len(),
            },
            targets: targets
                .into_iter()
                .map(|t| WorkspaceTargetSummary {
                    id: t.id,
                    project_id: t.project_id,
                    name: t.name,
                    target_type: t.target_type,
                })
                .collect(),
            scans: scans
                .into_iter()
                .map(|s| WorkspaceScanSummary {
                    id: s.id,
                    project_id: s.project_id,
                    name: s.name,
                    status: s.status,
                    target_id: s.target_id,
                })
                .collect(),
        })
    }

    async fn list_scan(&self, project: &str) -> Result<ScanList, String> {
        let project = self.resolve_project(project).await?;
        let scans = self
            .repos
            .scans()
            .list_by_project(&project.id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(ScanList {
            project_id: project.id,
            project_name: project.name,
            scans: scans
                .into_iter()
                .map(|s| WorkspaceScanSummary {
                    id: s.id,
                    project_id: s.project_id,
                    name: s.name,
                    status: s.status,
                    target_id: s.target_id,
                })
                .collect(),
        })
    }

    async fn scan_detail(&self, scan_id: &str) -> Result<ScanDetail, String> {
        let scan_id = scan_id.trim();
        if scan_id.is_empty() {
            return Err("scan_id is required".into());
        }
        let scan = self
            .repos
            .scans()
            .get(scan_id)
            .await
            .map_err(|e| e.to_string())?;
        let project_name = self
            .repos
            .projects()
            .get(&scan.project_id)
            .await
            .map(|p| p.name)
            .unwrap_or_else(|_| scan.project_id.clone());

        let mut findings = self
            .repos
            .findings()
            .list_by_scan(&scan.id)
            .await
            .map_err(|e| e.to_string())?;
        findings.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let findings_total = findings.len();
        let limit = DEFAULT_FINDINGS_LIMIT.min(MAX_FINDINGS_LIMIT);
        let findings_truncated = findings_total.saturating_sub(limit);
        findings.truncate(limit);

        Ok(ScanDetail {
            scan: WorkspaceScanSummary {
                id: scan.id,
                project_id: scan.project_id,
                name: scan.name,
                status: scan.status,
                target_id: scan.target_id,
            },
            project_name,
            findings: findings
                .into_iter()
                .map(|f| WorkspaceFindingSummary {
                    id: f.id,
                    project_id: f.project_id,
                    scan_id: f.scan_id,
                    title: f.title,
                    severity: f.severity,
                    status: f.status,
                    category: f.category,
                })
                .collect(),
            findings_total,
            findings_truncated,
        })
    }

    async fn list_findings(
        &self,
        project: Option<&str>,
        scan_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<FindingList, String> {
        let limit = clamp_findings_limit(Some(limit));
        let offset = offset;

        let (mut findings, project_id, project_name, scan_id_out) =
            if let Some(scan_id) = scan_id.map(str::trim).filter(|s| !s.is_empty()) {
                let scan = self
                    .repos
                    .scans()
                    .get(scan_id)
                    .await
                    .map_err(|e| e.to_string())?;
                let project_name = self
                    .repos
                    .projects()
                    .get(&scan.project_id)
                    .await
                    .map(|p| p.name)
                    .unwrap_or_else(|_| scan.project_id.clone());
                let findings = self
                    .repos
                    .findings()
                    .list_by_scan(scan_id)
                    .await
                    .map_err(|e| e.to_string())?;
                (
                    findings,
                    Some(scan.project_id),
                    Some(project_name),
                    Some(scan_id.to_string()),
                )
            } else if let Some(project) = project.map(str::trim).filter(|s| !s.is_empty()) {
                let project = self.resolve_project(project).await?;
                let findings = self
                    .repos
                    .findings()
                    .list_by_project(&project.id)
                    .await
                    .map_err(|e| e.to_string())?;
                (
                    findings,
                    Some(project.id),
                    Some(project.name),
                    None,
                )
            } else {
                return Err("list_findings requires project and/or scan_id".into());
            };

        findings.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let total = findings.len();
        let page: Vec<_> = findings.into_iter().skip(offset).take(limit).collect();

        Ok(FindingList {
            project_id,
            project_name,
            scan_id: scan_id_out,
            findings: page
                .into_iter()
                .map(|f| WorkspaceFindingSummary {
                    id: f.id,
                    project_id: f.project_id,
                    scan_id: f.scan_id,
                    title: f.title,
                    severity: f.severity,
                    status: f.status,
                    category: f.category,
                })
                .collect(),
            total,
            offset,
            limit,
        })
    }

    async fn finding_detail(
        &self,
        finding_id: Option<&str>,
        project: Option<&str>,
        index: Option<usize>,
    ) -> Result<FindingDetail, String> {
        if let Some(id) = finding_id.map(str::trim).filter(|s| !s.is_empty()) {
            let f = self
                .repos
                .findings()
                .get(id)
                .await
                .map_err(|e| e.to_string())?;
            let project_name = self
                .repos
                .projects()
                .get(&f.project_id)
                .await
                .map(|p| p.name)
                .unwrap_or_else(|_| f.project_id.clone());
            return Ok(FindingDetail {
                index: None,
                finding: WorkspaceFindingSummary {
                    id: f.id,
                    project_id: f.project_id,
                    scan_id: f.scan_id,
                    title: f.title,
                    severity: f.severity,
                    status: f.status,
                    category: f.category,
                },
                project_name,
                description: f.description,
                evidence_json: f.evidence_json,
            });
        }

        let project_key = project
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "finding_detail requires finding_id or project+index".to_string())?;
        let index = index
            .filter(|n| *n > 0)
            .ok_or_else(|| "finding_detail index must be >= 1".to_string())?;
        let project = self.resolve_project(project_key).await?;
        let mut findings = self
            .repos
            .findings()
            .list_by_project(&project.id)
            .await
            .map_err(|e| e.to_string())?;
        findings.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let f = findings.get(index - 1).ok_or_else(|| {
            format!(
                "no finding #{index} for project {} (total {})",
                project.name,
                findings.len()
            )
        })?;
        Ok(FindingDetail {
            index: Some(index),
            finding: WorkspaceFindingSummary {
                id: f.id.clone(),
                project_id: f.project_id.clone(),
                scan_id: f.scan_id.clone(),
                title: f.title.clone(),
                severity: f.severity.clone(),
                status: f.status.clone(),
                category: f.category.clone(),
            },
            project_name: project.name,
            description: f.description.clone(),
            evidence_json: f.evidence_json.clone(),
        })
    }

    async fn list_targets(&self, project: &str) -> Result<TargetList, String> {
        let project = self.resolve_project(project).await?;
        let targets = self
            .repos
            .targets()
            .list_by_project(&project.id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(TargetList {
            project_id: project.id,
            project_name: project.name,
            targets: targets
                .into_iter()
                .map(|t| WorkspaceTargetSummary {
                    id: t.id,
                    project_id: t.project_id,
                    name: t.name,
                    target_type: t.target_type,
                })
                .collect(),
        })
    }

    async fn target_detail(
        &self,
        target_id: Option<&str>,
        project: Option<&str>,
        name: Option<&str>,
    ) -> Result<TargetDetail, String> {
        let id_key = target_id.map(str::trim).filter(|s| !s.is_empty());
        let name_key = name.map(str::trim).filter(|s| !s.is_empty());
        let project_key = project.map(str::trim).filter(|s| !s.is_empty());

        // Models routinely pass a host/name in `target_id`, so an id miss falls
        // back to name/host matching instead of hard-failing.
        let mut target = match id_key {
            Some(id) => self.repos.targets().get(id).await.ok(),
            None => None,
        };

        if target.is_none() {
            let lookup = name_key.or(id_key).ok_or_else(|| {
                "target_detail requires target_id or project+name".to_string()
            })?;
            let candidates = match project_key {
                Some(key) => {
                    let project = self.resolve_project(key).await?;
                    self.repos
                        .targets()
                        .list_by_project(&project.id)
                        .await
                        .map_err(|e| e.to_string())?
                }
                None => self
                    .repos
                    .targets()
                    .list_all()
                    .await
                    .map_err(|e| e.to_string())?,
            };
            target = match_target(&candidates, lookup);
            if target.is_none() {
                return Err(match project_key {
                    Some(key) => format!("target not found: {lookup} in project {key}"),
                    None => format!("target not found: {lookup}"),
                });
            }
        }

        let target = target.ok_or_else(|| "target not found".to_string())?;

        let project_name = self
            .repos
            .projects()
            .get(&target.project_id)
            .await
            .map(|p| p.name)
            .unwrap_or_else(|_| target.project_id.clone());

        let descriptor = parse_json_blob(&target.descriptor_json);
        let profile = parse_json_blob(&target.profile_json);
        let endpoint = descriptor
            .as_ref()
            .and_then(extract_endpoint)
            .or_else(|| profile.as_ref().and_then(extract_endpoint));
        let host = endpoint
            .as_deref()
            .and_then(host_from_endpoint)
            .or_else(|| Some(target.name.clone()));
        let verified = profile
            .as_ref()
            .and_then(|p| p.get("verification"))
            .and_then(|v| v.get("verified"))
            .and_then(|v| v.as_bool());

        Ok(TargetDetail {
            target: WorkspaceTargetSummary {
                id: target.id,
                project_id: target.project_id,
                name: target.name,
                target_type: target.target_type,
            },
            project_name,
            endpoint,
            host,
            verified,
        })
    }

    async fn list_reports(&self, project: Option<&str>) -> Result<ReportList, String> {
        let (reports, project_id, project_name) =
            if let Some(project) = project.map(str::trim).filter(|s| !s.is_empty()) {
                let project = self.resolve_project(project).await?;
                let reports = self
                    .repos
                    .reports()
                    .list_by_project(&project.id)
                    .await
                    .map_err(|e| e.to_string())?;
                (reports, Some(project.id), Some(project.name))
            } else {
                // Aggregate across projects (no list_all on trait — loop projects).
                let projects = self
                    .repos
                    .projects()
                    .list()
                    .await
                    .map_err(|e| e.to_string())?;
                let mut all = Vec::new();
                for p in projects {
                    let mut rows = self
                        .repos
                        .reports()
                        .list_by_project(&p.id)
                        .await
                        .map_err(|e| e.to_string())?;
                    all.append(&mut rows);
                }
                (all, None, None)
            };

        Ok(ReportList {
            project_id,
            project_name,
            reports: reports
                .into_iter()
                .map(|r| WorkspaceReportSummary {
                    finding_count: finding_count_from_report_metadata(r.metadata_json.as_deref()),
                    id: r.id,
                    project_id: r.project_id,
                    scan_id: r.scan_id,
                    name: r.name,
                    format: r.format,
                    status: r.status,
                })
                .collect(),
        })
    }

    async fn report_detail(&self, report_id: &str) -> Result<ReportDetail, String> {
        let report_id = report_id.trim();
        if report_id.is_empty() {
            return Err("report_id is required".into());
        }
        let report = self
            .repos
            .reports()
            .get(report_id)
            .await
            .map_err(|e| e.to_string())?;
        let project_name = self
            .repos
            .projects()
            .get(&report.project_id)
            .await
            .map(|p| p.name)
            .unwrap_or_else(|_| report.project_id.clone());

        let (content_preview, content_truncated) = match report.file_path.as_deref() {
            Some(path) => match std::fs::read_to_string(path) {
                Ok(raw) => clip_text(&raw, MAX_REPORT_PREVIEW_CHARS),
                Err(err) => (format!("(failed to read report file: {err})"), false),
            },
            None => ("(report has no saved file)".into(), false),
        };

        Ok(ReportDetail {
            report: WorkspaceReportSummary {
                finding_count: finding_count_from_report_metadata(report.metadata_json.as_deref()),
                id: report.id,
                project_id: report.project_id,
                scan_id: report.scan_id,
                name: report.name,
                format: report.format,
                status: report.status,
            },
            project_name,
            file_path: report.file_path,
            content_preview,
            content_truncated,
        })
    }
}

/// Resolve a target by id, exact name, host, or substring — in that order, so a
/// host like `10.100.109.76` still finds its target.
fn match_target(
    candidates: &[promptlab_storage::Target],
    lookup: &str,
) -> Option<promptlab_storage::Target> {
    let key = lookup.trim();
    if key.is_empty() {
        return None;
    }
    if let Some(t) = candidates.iter().find(|t| t.id == key) {
        return Some(t.clone());
    }
    let lower = key.to_lowercase();
    if let Some(t) = candidates
        .iter()
        .find(|t| t.name.to_lowercase() == lower)
    {
        return Some(t.clone());
    }
    // Match against the host inside the descriptor/profile endpoint.
    if let Some(t) = candidates.iter().find(|t| {
        [t.descriptor_json.as_str(), t.profile_json.as_str()]
            .iter()
            .filter_map(|raw| parse_json_blob(raw))
            .filter_map(|v| extract_endpoint(&v))
            .any(|endpoint| {
                host_from_endpoint(&endpoint)
                    .map(|host| host.to_lowercase() == lower)
                    .unwrap_or(false)
            })
    }) {
        return Some(t.clone());
    }
    candidates
        .iter()
        .find(|t| {
            let n = t.name.to_lowercase();
            n.contains(&lower) || lower.contains(&n)
        })
        .cloned()
}

fn parse_json_blob(raw: &str) -> Option<serde_json::Value> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "{}" {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

fn extract_endpoint(value: &serde_json::Value) -> Option<String> {
    let obj = value.as_object()?;
    for key in ["fullUrl", "full_url", "baseUrl", "base_url", "url", "endpoint", "host"] {
        if let Some(s) = obj.get(key).and_then(|v| v.as_str()) {
            let s = s.trim();
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn host_from_endpoint(endpoint: &str) -> Option<String> {
    let raw = endpoint.trim();
    if raw.is_empty() {
        return None;
    }
    let without_scheme = raw.split_once("://").map(|(_, rest)| rest).unwrap_or(raw);
    let authority = without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(without_scheme);
    let host = authority.rsplit_once('@').map(|(_, h)| h).unwrap_or(authority);
    let host = host.split(':').next().unwrap_or(host).trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

fn clip_text(raw: &str, max_chars: usize) -> (String, bool) {
    let count = raw.chars().count();
    if count <= max_chars {
        return (raw.to_string(), false);
    }
    let clipped: String = raw.chars().take(max_chars).collect();
    (format!("{clipped}…"), true)
}

fn finding_count_from_report_metadata(metadata_json: Option<&str>) -> u64 {
    metadata_json
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|v| v.get("findings").and_then(|f| f.as_u64()))
        .unwrap_or(0)
}

fn profile_from_json(raw: &str) -> CommandResult<TargetProfile> {
    if raw.trim().is_empty() || raw == "{}" {
        return Ok(TargetProfile::default());
    }
    serde_json::from_str(raw).map_err(|err| {
        CommandError::invalid_input(format!("invalid target profile: {err}"))
    })
}

fn event_dto(event: AgentEvent) -> AgentEventDto {
    AgentEventDto {
        agent: event.agent.as_str().into(),
        kind: match event.kind {
            promptlab_agent::AgentEventKind::Started => "started".into(),
            promptlab_agent::AgentEventKind::Completed => "completed".into(),
            promptlab_agent::AgentEventKind::Failed => "failed".into(),
            promptlab_agent::AgentEventKind::Info => "info".into(),
            promptlab_agent::AgentEventKind::React => "react".into(),
            promptlab_agent::AgentEventKind::ToolCall => "tool_call".into(),
            promptlab_agent::AgentEventKind::Llm => "llm".into(),
        },
        message: event.message,
    }
}

fn hilt_pending_dto(pending: HiltPendingAction) -> YazgHiltPendingActionDto {
    YazgHiltPendingActionDto {
        id: pending.id,
        tool: pending.tool,
        kind: pending.kind.as_str().into(),
        args: pending.args,
        summary: pending.summary,
        created_at_ms: pending.created_at_ms,
        expires_at_ms: pending.expires_at_ms,
    }
}

fn turn_to_response(
    turn: YazgTurn,
    created_project: Option<CreatedProject>,
) -> YazgChatResponse {
    let raw_output = turn
        .raw_output
        .clone()
        .or_else(|| serde_json::to_value(&turn).ok());
    YazgChatResponse {
        reply: turn.reply,
        intent: match turn.intent {
            SupervisorIntent::Auto => "auto".into(),
            SupervisorIntent::Chat => "chat".into(),
            SupervisorIntent::AnalyzeEndpoint => "analyze_endpoint".into(),
            SupervisorIntent::AttackPlan => "attack_plan".into(),
            SupervisorIntent::GeneratePrompt => "generate_prompt".into(),
            SupervisorIntent::Recommend => "recommend".into(),
            SupervisorIntent::Summary => "summary".into(),
            SupervisorIntent::Judge => "judge".into(),
            SupervisorIntent::ExecuteAttack => "execute_attack".into(),
            SupervisorIntent::CreateProject => "create_project".into(),
            SupervisorIntent::ListWorkspace => "list_workspace".into(),
        },
        action: turn.action,
        events: turn.events.into_iter().map(event_dto).collect(),
        raw_output,
        verified: turn.verified,
        plan_summary: turn.plan_summary,
        created_project: created_project.map(|project| YazgCreatedProjectDto {
            id: project.id,
            name: project.name,
            description: project.description,
        }),
        pending_action: turn.pending_action.map(hilt_pending_dto),
        trace_id: turn.trace_id,
    }
}

fn map_agent_err(err: promptlab_agent::AgentError) -> CommandError {
    CommandError::invalid_input(err.to_string())
}

pub async fn yazg_chat_op(
    state: &AppState,
    request: YazgChatRequest,
) -> CommandResult<YazgChatResponse> {
    let intent_hint = SupervisorIntent::parse(request.intent.as_deref().unwrap_or("auto"));

    let (profile, auth_headers, project_id) = if let Some(target_id) = request.target_id.as_ref() {
        let target = state
            .repositories()
            .targets()
            .get(target_id)
            .await
            .map_err(CommandError::from)?;
        let profile = profile_from_json(&target.profile_json)?;
        let auth_headers =
            crate::commands::target_profile::auth_headers_from_descriptor(&target.descriptor_json)?;
        (Some(profile), auth_headers, Some(target.project_id))
    } else {
        (None, HashMap::new(), None)
    };

    let inference = state.inference_manager().lock().await;
    let runtime_ready = is_inference_ready(&inference);
    let model_label = {
        let name = inference.config().model.trim();
        if name.is_empty() {
            inference
                .config()
                .selected_model_id
                .clone()
                .filter(|s| !s.trim().is_empty())
        } else {
            Some(name.to_string())
        }
    };
    drop(inference);

    if !runtime_ready {
        return Ok(turn_to_response(
            YazgSupervisor::offline_chat(&request.message, profile.as_ref()),
            None,
        ));
    }

    let hosts = YazgHostLlms::from_app(
        state.data_dir().to_path_buf(),
        state.inference_manager().clone(),
        state.model_manager().clone(),
        state.model_provider().clone(),
        state.runtime_manager().clone(),
    );
    let llms = hosts.into_yazg_llms();
    let memory: Arc<dyn promptlab_agent::AgentMemoryStore> =
        Arc::new(SqliteAgentMemoryStore::new(state.repositories()));
    let project_tools: Arc<dyn CreateProjectTools> = Arc::new(HostCreateProjectTools {
        repos: state.repositories(),
    });
    let workspace_tools: Arc<dyn WorkspaceTools> = Arc::new(HostWorkspaceTools {
        repos: state.repositories(),
    });
    let session_id = request
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("yazg-chat:assistant")
        .to_string();
    // One conversation thread → one STM session.
    let memory_ctx = MemoryContext::new(session_id)
        .with_project(project_id)
        .with_target(request.target_id.clone());

    let delegation = YazgSupervisor::handle(
        &request.message,
        intent_hint,
        profile.as_ref(),
        auth_headers,
        llms,
        Some(memory),
        memory_ctx,
        Some(project_tools),
        Some(workspace_tools),
        Some(state.agent_trace().clone()),
        model_label,
    )
    .await
    .map_err(map_agent_err)?;

    if let (Some(target_id), YazgDelegation::AnalyzedEndpoint { outcome, turn }) =
        (request.target_id.as_ref(), &delegation)
    {
        if let Ok(target) = state.repositories().targets().get(target_id).await {
            if let Ok(mut profile) = profile_from_json(&target.profile_json) {
                profile.verification = outcome.verification.clone();
                if let Ok(json) = serde_json::to_string(&profile) {
                    let _ = state
                        .repositories()
                        .targets()
                        .update_profile(target_id, &json)
                        .await;
                }
            }
        }
        return Ok(turn_to_response(turn.clone(), None));
    }

    let (turn, created_project) = match delegation {
        YazgDelegation::CreatedProject { turn, project } => (turn, Some(project)),
        YazgDelegation::Chat { turn }
        | YazgDelegation::AnalyzedEndpoint { turn, .. }
        | YazgDelegation::Planned { turn, .. }
        | YazgDelegation::GeneratedPrompt { turn, .. }
        | YazgDelegation::Recommended { turn, .. }
        | YazgDelegation::Summarized { turn, .. }
        | YazgDelegation::Judged { turn, .. }
        | YazgDelegation::ExecutedAttack { turn, .. } => (turn, None),
    };
    let response = turn_to_response(turn, created_project);
    if let Some(pending) = response.pending_action.as_ref() {
        remember_hilt_pending(HiltPendingAction {
            id: pending.id.clone(),
            tool: pending.tool.clone(),
            kind: match pending.kind.as_str() {
                "update" => promptlab_agent::HiltMutationKind::Update,
                "delete" => promptlab_agent::HiltMutationKind::Delete,
                _ => promptlab_agent::HiltMutationKind::Create,
            },
            args: pending.args.clone(),
            summary: pending.summary.clone(),
            created_at_ms: pending.created_at_ms,
            expires_at_ms: pending.expires_at_ms,
        })
        .await;
    }
    Ok(response)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YazgGenerateChatTitleRequest {
    pub message: String,
    #[serde(default)]
    pub reply: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YazgGenerateChatTitleResponse {
    pub title: String,
}

fn fallback_chat_title(message: &str) -> String {
    let trimmed = message.trim().replace('\n', " ");
    let collapsed = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return "New chat".into();
    }
    let words: Vec<&str> = collapsed.split_whitespace().take(8).collect();
    let title = words.join(" ");
    if collapsed.split_whitespace().count() > 8 {
        format!("{title}…")
    } else {
        title
    }
}

fn sanitize_chat_title(raw: &str, fallback: &str) -> String {
    let mut title = raw
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches(|c| c == '"' || c == '\'' || c == '`' || c == '*')
        .trim()
        .to_string();
    // Drop common prefixes models add.
    for prefix in ["Title:", "title:", "Conversation title:", "Chat title:"] {
        if let Some(rest) = title.strip_prefix(prefix) {
            title = rest.trim().to_string();
        }
    }
    title = title
        .trim_matches(|c| c == '"' || c == '\'' || c == '`' || c == '*' || c == '.')
        .trim()
        .to_string();
    let words: Vec<&str> = title.split_whitespace().take(8).collect();
    if words.is_empty() {
        return fallback.to_string();
    }
    words.join(" ")
}

pub async fn yazg_generate_chat_title_op(
    state: &AppState,
    request: YazgGenerateChatTitleRequest,
) -> CommandResult<YazgGenerateChatTitleResponse> {
    let message = request.message.trim();
    let fallback = fallback_chat_title(message);
    if message.is_empty() {
        return Ok(YazgGenerateChatTitleResponse { title: fallback });
    }

    let inference = state.inference_manager().lock().await;
    let runtime_ready = is_inference_ready(&inference);
    drop(inference);
    if !runtime_ready {
        return Ok(YazgGenerateChatTitleResponse { title: fallback });
    }

    let mut prompt = format!(
        "Generate a short, concise title (maximum 6-8 words) that capture the main topic. \
         Return only the title text nothing else. Do not use quotes.\n\n\
         User message:\n{message}\n"
    );
    if let Some(reply) = request
        .reply
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let clipped: String = reply.chars().take(400).collect();
        prompt.push_str(&format!("\nAssistant reply:\n{clipped}\n"));
    }

    let inference = state.inference_manager().lock().await;
    let manager = state.model_manager().lock().await;
    let mut runtime_mgr = state.runtime_manager().lock().await;
    match gateway_complete_as(
        state.data_dir(),
        &inference,
        &manager,
        state.model_provider().clone(),
        &mut runtime_mgr,
        "yazg",
        None,
        &prompt,
        32,
        0.2,
    )
    .await
    {
        Ok(raw) => Ok(YazgGenerateChatTitleResponse {
            title: sanitize_chat_title(&raw, &fallback),
        }),
        Err(_) => Ok(YazgGenerateChatTitleResponse { title: fallback }),
    }
}

#[tauri::command]
pub async fn yazg_chat(
    state: State<'_, AppState>,
    request: YazgChatRequest,
) -> CommandResult<YazgChatResponse> {
    yazg_chat_op(state.inner(), request).await
}

#[tauri::command]
pub async fn yazg_generate_chat_title(
    state: State<'_, AppState>,
    request: YazgGenerateChatTitleRequest,
) -> CommandResult<YazgGenerateChatTitleResponse> {
    yazg_generate_chat_title_op(state.inner(), request).await
}

// ─── HILT (human-in-the-loop) for mutating tools ─────────────────────────────

fn hilt_store() -> &'static TokioMutex<HashMap<String, HiltPendingAction>> {
    static STORE: OnceLock<TokioMutex<HashMap<String, HiltPendingAction>>> = OnceLock::new();
    STORE.get_or_init(|| TokioMutex::new(HashMap::new()))
}

async fn remember_hilt_pending(pending: HiltPendingAction) {
    let mut store = hilt_store().lock().await;
    prune_expired_hilt(&mut store);
    store.insert(pending.id.clone(), pending);
}

fn prune_expired_hilt(store: &mut HashMap<String, HiltPendingAction>) {
    store.retain(|_, pending| !pending.is_expired());
}

async fn take_hilt_pending(action_id: &str) -> Option<HiltPendingAction> {
    let mut store = hilt_store().lock().await;
    // Keep the requested id even if expired so deny/expire can still clear it;
    // prune everything else.
    let requested = store.remove(action_id);
    prune_expired_hilt(&mut store);
    requested
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YazgResolveHiltRequest {
    pub action_id: String,
    /// `approve` | `deny` | `expire`
    pub decision: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

fn hilt_decision_label(decision: &str, expired: bool) -> &'static str {
    if expired || decision == "expire" {
        "expire"
    } else if decision == "approve" || decision == "accept" || decision == "confirm" {
        "approve"
    } else {
        "deny"
    }
}

fn hilt_static_fallback(
    pending: &HiltPendingAction,
    decision: &str,
    created_project: Option<&CreatedProject>,
) -> YazgChatResponse {
    let reply = match decision {
        "approve" => {
            if let Some(created) = created_project {
                format!("Created project \"{}\" (id={}).", created.name, created.id)
            } else {
                format!("Approved: {}.", pending.summary)
            }
        }
        "expire" => format!("Hết hạn 15 phút — đã tự hủy: {}.", pending.summary),
        _ => format!("Cancelled: {}.", pending.summary),
    };
    YazgChatResponse {
        reply,
        intent: pending.tool.clone(),
        action: Some(pending.tool.clone()),
        events: vec![AgentEventDto {
            agent: "yazg".into(),
            kind: if decision == "approve" {
                "completed".into()
            } else {
                "info".into()
            },
            message: format!("hilt_{decision}:{}", pending.id),
        }],
        raw_output: None,
        verified: None,
        plan_summary: None,
        created_project: created_project.map(|project| YazgCreatedProjectDto {
            id: project.id.clone(),
            name: project.name.clone(),
            description: project.description.clone(),
        }),
        pending_action: None,
        trace_id: None,
    }
}

async fn run_hilt_followup_turn(
    state: &AppState,
    pending: HiltPendingAction,
    decision: &str,
    tool_observation: Option<String>,
    created_project: Option<CreatedProject>,
    session_id: Option<String>,
) -> CommandResult<YazgChatResponse> {
    let inference = state.inference_manager().lock().await;
    let runtime_ready = is_inference_ready(&inference);
    let model_label = {
        let name = inference.config().model.trim();
        if name.is_empty() {
            inference
                .config()
                .selected_model_id
                .clone()
                .filter(|s| !s.trim().is_empty())
        } else {
            Some(name.to_string())
        }
    };
    drop(inference);

    if !runtime_ready {
        return Ok(hilt_static_fallback(
            &pending,
            decision,
            created_project.as_ref(),
        ));
    }

    let hosts = YazgHostLlms::from_app(
        state.data_dir().to_path_buf(),
        state.inference_manager().clone(),
        state.model_manager().clone(),
        state.model_provider().clone(),
        state.runtime_manager().clone(),
    );
    let llms = hosts.into_yazg_llms();
    let memory: Arc<dyn promptlab_agent::AgentMemoryStore> =
        Arc::new(SqliteAgentMemoryStore::new(state.repositories()));
    let session_id = session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("yazg-chat:assistant")
        .to_string();
    let memory_ctx = MemoryContext::new(session_id);

    let turn = YazgSupervisor::hilt_followup(
        &pending,
        decision,
        tool_observation.as_deref(),
        llms,
        Some(memory),
        memory_ctx,
        Some(state.agent_trace().clone()),
        model_label,
    )
    .await
    .map_err(map_agent_err)?;

    Ok(turn_to_response(turn, created_project))
}

pub async fn yazg_resolve_hilt_op(
    state: &AppState,
    request: YazgResolveHiltRequest,
) -> CommandResult<YazgChatResponse> {
    let action_id = request.action_id.trim();
    if action_id.is_empty() {
        return Err(CommandError::invalid_input("actionId is required"));
    }
    let decision_raw = request.decision.trim().to_lowercase();
    let pending = take_hilt_pending(action_id).await.ok_or_else(|| {
        CommandError::invalid_input(format!(
            "no pending HILT action for id `{action_id}` (already resolved or expired)"
        ))
    })?;

    if !is_mutating_tool(&pending.tool) {
        return Err(CommandError::invalid_input(format!(
            "tool `{}` is not a mutating HILT tool",
            pending.tool
        )));
    }

    let expired = pending.is_expired();
    let decision = hilt_decision_label(&decision_raw, expired);

    if decision == "deny" || decision == "expire" {
        return run_hilt_followup_turn(
            state,
            pending,
            decision,
            None,
            None,
            request.session_id,
        )
        .await;
    }

    if decision != "approve" {
        remember_hilt_pending(pending).await;
        return Err(CommandError::invalid_input(
            "decision must be `approve` or `deny`",
        ));
    }

    match pending.tool.as_str() {
        "create_project" => {
            let name = pending
                .args
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    CommandError::invalid_input("pending create_project is missing name")
                })?;
            let description = pending
                .args
                .get("description")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let tools = HostCreateProjectTools {
                repos: state.repositories(),
            };
            let created = tools
                .create_project(name, description)
                .await
                .map_err(CommandError::invalid_input)?;
            let observation = ToolResult::ok(
                "create_project",
                serde_json::json!({
                    "id": created.id,
                    "name": created.name,
                    "description": created.description,
                }),
            )
            .to_json_string();
            run_hilt_followup_turn(
                state,
                pending,
                "approve",
                Some(observation),
                Some(created),
                request.session_id,
            )
            .await
        }
        other => Err(CommandError::invalid_input(format!(
            "HILT resolve not implemented for tool `{other}`"
        ))),
    }
}

#[tauri::command]
pub async fn yazg_resolve_hilt(
    state: State<'_, AppState>,
    request: YazgResolveHiltRequest,
) -> CommandResult<YazgChatResponse> {
    yazg_resolve_hilt_op(state.inner(), request).await
}
