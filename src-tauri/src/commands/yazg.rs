//! Yazg supervisor chat IPC (ReAct).

use std::collections::HashMap;

use promptlab_agent::{
    AgentEvent, CreateProjectTools, CreatedProject, MemoryContext, SupervisorIntent,
    WorkspaceFindingSummary, WorkspaceInventory, WorkspaceProjectSummary, WorkspaceScanSummary,
    WorkspaceTargetSummary, WorkspaceTools, WorkspaceTotals, YazgDelegation, YazgSupervisor,
    YazgTurn,
};
use promptlab_storage::{
    FindingRepository, ProjectRepository, ScanRepository, TargetRepository,
};
use promptlab_target_profile::TargetProfile;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::agent_memory::SqliteAgentMemoryStore;
use crate::commands::projects::project_create_op;
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

struct HostCreateProjectTools<'a> {
    state: &'a AppState,
}

#[async_trait]
impl CreateProjectTools for HostCreateProjectTools<'_> {
    async fn create_project(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<CreatedProject, String> {
        project_create_op(
            self.state,
            name.to_string(),
            description.map(str::to_string),
        )
        .await
        .map(|project| CreatedProject {
            id: project.id,
            name: project.name,
            description: project.description,
        })
        .map_err(|err| err.to_string())
    }
}

struct HostWorkspaceTools<'a> {
    state: &'a AppState,
}

const MAX_LISTED_FINDINGS: usize = 40;

#[async_trait]
impl WorkspaceTools for HostWorkspaceTools<'_> {
    async fn list_workspace(&self) -> Result<WorkspaceInventory, String> {
        let repos = self.state.repositories();

        let projects = repos
            .projects()
            .list()
            .await
            .map_err(|err| err.to_string())?;
        let targets = repos
            .targets()
            .list_all()
            .await
            .map_err(|err| err.to_string())?;

        let mut scans = Vec::new();
        let mut findings = Vec::new();
        for project in &projects {
            let project_scans = repos
                .scans()
                .list_by_project(&project.id)
                .await
                .map_err(|err| err.to_string())?;
            scans.extend(project_scans);

            let project_findings = repos
                .findings()
                .list_by_project(&project.id)
                .await
                .map_err(|err| err.to_string())?;
            findings.extend(project_findings);
        }

        // Newest findings first when truncating the listed set.
        findings.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let findings_total = findings.len();
        let findings_truncated = findings_total.saturating_sub(MAX_LISTED_FINDINGS);
        findings.truncate(MAX_LISTED_FINDINGS);

        Ok(WorkspaceInventory {
            totals: WorkspaceTotals {
                projects: projects.len(),
                targets: targets.len(),
                scans: scans.len(),
                findings: findings_total,
                findings_truncated,
            },
            projects: projects
                .into_iter()
                .map(|project| WorkspaceProjectSummary {
                    id: project.id,
                    name: project.name,
                    description: project.description,
                })
                .collect(),
            targets: targets
                .into_iter()
                .map(|target| WorkspaceTargetSummary {
                    id: target.id,
                    project_id: target.project_id,
                    name: target.name,
                    target_type: target.target_type,
                })
                .collect(),
            scans: scans
                .into_iter()
                .map(|scan| WorkspaceScanSummary {
                    id: scan.id,
                    project_id: scan.project_id,
                    name: scan.name,
                    status: scan.status,
                    target_id: scan.target_id,
                })
                .collect(),
            findings: findings
                .into_iter()
                .map(|finding| WorkspaceFindingSummary {
                    id: finding.id,
                    project_id: finding.project_id,
                    scan_id: finding.scan_id,
                    title: finding.title,
                    severity: finding.severity,
                    status: finding.status,
                    category: finding.category,
                })
                .collect(),
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
    let llms = hosts.react_llms();
    let memory = SqliteAgentMemoryStore::new(state.repositories());
    let project_tools = HostCreateProjectTools { state };
    let workspace_tools = HostWorkspaceTools { state };
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
        &llms,
        Some(&memory),
        memory_ctx,
        Some(&project_tools),
        Some(&workspace_tools),
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
