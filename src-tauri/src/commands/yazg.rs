//! Yazg supervisor chat IPC (ReAct).

use std::collections::HashMap;

use aisec_agent::{
    AgentEvent, SupervisorIntent, YazgDelegation, YazgSupervisor, YazgTurn,
};
use aisec_core::AisecError;
use aisec_storage::TargetRepository;
use aisec_target_profile::TargetProfile;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::inference_host::{
    is_inference_ready, HostEndpointVerifyLlm, HostWizardPlannerLlm, HostYazgReactLlm,
};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YazgChatRequest {
    pub message: String,
    #[serde(default)]
    pub target_id: Option<String>,
    /// Soft hint only — Yazg ReAct still chooses the action.
    /// `auto` | `chat` | `analyze_endpoint` | `verify` (alias) | `attack_plan` | `plan` (alias)
    #[serde(default)]
    pub intent: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YazgChatResponse {
    pub reply: String,
    pub intent: String,
    pub events: Vec<AgentEventDto>,
    pub verified: Option<bool>,
    pub plan_summary: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEventDto {
    pub agent: String,
    pub kind: String,
    pub message: String,
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
            aisec_agent::AgentEventKind::Started => "started".into(),
            aisec_agent::AgentEventKind::Completed => "completed".into(),
            aisec_agent::AgentEventKind::Failed => "failed".into(),
            aisec_agent::AgentEventKind::Info => "info".into(),
        },
        message: event.message,
    }
}

fn turn_to_response(turn: YazgTurn) -> YazgChatResponse {
    YazgChatResponse {
        reply: turn.reply,
        intent: match turn.intent {
            SupervisorIntent::Auto => "auto".into(),
            SupervisorIntent::Chat => "chat".into(),
            SupervisorIntent::AnalyzeEndpoint => "analyze_endpoint".into(),
            SupervisorIntent::AttackPlan => "attack_plan".into(),
        },
        events: turn.events.into_iter().map(event_dto).collect(),
        verified: turn.verified,
        plan_summary: turn.plan_summary,
    }
}

fn map_agent_err(err: aisec_agent::AgentError) -> CommandError {
    CommandError::invalid_input(err.to_string())
}

pub async fn yazg_chat_op(
    state: &AppState,
    request: YazgChatRequest,
) -> CommandResult<YazgChatResponse> {
    let intent_hint = SupervisorIntent::parse(request.intent.as_deref().unwrap_or("auto"));

    let (profile, auth_headers) = if let Some(target_id) = request.target_id.as_ref() {
        let target = state
            .repositories()
            .targets()
            .get(target_id)
            .await
            .map_err(CommandError::from)?;
        let profile = profile_from_json(&target.profile_json)?;
        let auth_headers =
            crate::commands::target_profile::auth_headers_from_descriptor(&target.descriptor_json)?;
        (Some(profile), auth_headers)
    } else {
        (None, HashMap::new())
    };

    let inference = state.inference_manager().lock().await;
    let runtime_ready = is_inference_ready(&inference);
    drop(inference);

    if !runtime_ready {
        return Ok(turn_to_response(YazgSupervisor::offline_chat(
            &request.message,
            profile.as_ref(),
        )));
    }

    let supervisor_llm = HostYazgReactLlm::new(
        state.data_dir().to_path_buf(),
        state.inference_manager().clone(),
        state.model_manager().clone(),
        state.model_provider().clone(),
        state.runtime_manager().clone(),
    );
    let analyze_llm = HostEndpointVerifyLlm::new(
        state.data_dir().to_path_buf(),
        state.inference_manager().clone(),
        state.model_manager().clone(),
        state.model_provider().clone(),
        state.runtime_manager().clone(),
    );
    let plan_llm = HostWizardPlannerLlm::new(
        state.data_dir().to_path_buf(),
        state.inference_manager().clone(),
        state.model_manager().clone(),
        state.model_provider().clone(),
        state.runtime_manager().clone(),
    );

    let delegation = YazgSupervisor::handle(
        &request.message,
        intent_hint,
        profile.as_ref(),
        auth_headers,
        &supervisor_llm,
        &analyze_llm,
        &plan_llm,
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
        return Ok(turn_to_response(turn.clone()));
    }

    let turn = match delegation {
        YazgDelegation::Chat { turn }
        | YazgDelegation::AnalyzedEndpoint { turn, .. }
        | YazgDelegation::Planned { turn, .. } => turn,
    };
    Ok(turn_to_response(turn))
}

#[tauri::command]
pub async fn yazg_chat(
    state: State<'_, AppState>,
    request: YazgChatRequest,
) -> CommandResult<YazgChatResponse> {
    yazg_chat_op(state.inner(), request).await
}
