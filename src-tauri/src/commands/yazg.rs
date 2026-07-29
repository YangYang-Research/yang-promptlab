//! Yazg supervisor chat IPC (ReAct).

use std::collections::HashMap;

use promptlab_agent::{
    AgentEvent, CreateProjectTools, CreatedProject, MemoryContext, SupervisorIntent,
    WorkspaceFindingSummary, WorkspaceInventory, WorkspaceProjectSummary, WorkspaceScanSummary,
    WorkspaceTargetSummary, WorkspaceTools, WorkspaceTotals, YazgDelegation, YazgSupervisor,
    YazgTurn, FindingDetail, FindingList, ProjectDetail, ScanDetail, ScanList,
    DEFAULT_FINDINGS_LIMIT, MAX_FINDINGS_LIMIT, clamp_findings_limit,
};
use promptlab_storage::{
    CreateProject, FindingRepository, ProjectRepository, ScanRepository, TargetRepository,
};
use promptlab_target_profile::TargetProfile;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tauri::State;

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
pub struct YazgChatResponse {
    pub reply: String,
    pub intent: String,
    pub events: Vec<AgentEventDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<Value>,
    pub verified: Option<bool>,
    pub plan_summary: Option<String>,
    pub created_project: Option<YazgCreatedProjectDto>,
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
        let mut matches: Vec<_> = projects
            .into_iter()
            .filter(|p| p.name.to_lowercase() == lower || p.name.to_lowercase().contains(&lower))
            .collect();
        matches.sort_by(|a, b| a.name.len().cmp(&b.name.len()));
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
        events: turn.events.into_iter().map(event_dto).collect(),
        raw_output,
        verified: turn.verified,
        plan_summary: turn.plan_summary,
        created_project: created_project.map(|project| YazgCreatedProjectDto {
            id: project.id,
            name: project.name,
            description: project.description,
        }),
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
    Ok(turn_to_response(turn, created_project))
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
