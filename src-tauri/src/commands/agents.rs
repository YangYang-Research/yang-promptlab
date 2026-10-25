//! Read-only agent catalog plus the stored attempt budget.

use serde::{Deserialize, Serialize};
use tauri::State;

use promptlab_agent::snapshot_agent_attempts;
use promptlab_inference::snapshot_agent_temperatures;
use promptlab_registry::agent_registry;

use crate::attempt_persist::{persist_agent_attempt, persist_agent_temperature};
use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSummaryDto {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub prompt_count: usize,
    pub tool_count: usize,
    pub attempt: Option<u32>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPromptDto {
    pub id: String,
    pub system: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentToolDto {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDetailDto {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub prompts: Vec<AgentPromptDto>,
    pub tools: Vec<AgentToolDto>,
    pub attempt: Option<u32>,
    pub attempt_note: Option<String>,
    pub temperature: Option<f32>,
    pub temperature_note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentGetRequest {
    pub agent_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSetAttemptRequest {
    pub agent_id: String,
    pub attempt: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSetTemperatureRequest {
    pub agent_id: String,
    pub temperature: Option<f32>,
}

fn temperature_fields(agent_id: &str, calls_llm: bool) -> (Option<f32>, Option<String>) {
    if !calls_llm {
        return (None, Some("This agent does not call an LLM.".into()));
    }
    match snapshot_agent_temperatures().get(agent_id).copied() {
        Some(temperature) => (Some(temperature), None),
        None => (
            None,
            Some("Uses the model temperature until set.".into()),
        ),
    }
}
fn attempt_fields(agent_id: &str) -> (Option<u32>, Option<String>) {
    if agent_id == promptlab_agent::AgentId::AgenticAttackExecution.as_str() {
        return (
            None,
            Some("Agentic scan uses maxAttempts from the attack plan.".into()),
        );
    }
    match snapshot_agent_attempts().get(agent_id).copied() {
        Some(count) => (Some(count), None),
        None => (None, Some("This agent has no attempt budget.".into())),
    }
}

fn detail_for(agent_id: &str) -> CommandResult<AgentDetailDto> {
    let agent = agent_registry::load(agent_id)
        .map_err(|_| CommandError::invalid_input(format!("unknown agent: {agent_id}")))?;
    let prompts = agent
        .prompt_ids()
        .iter()
        .map(|id| {
            let prompt = agent.prompt(id).map_err(|err| CommandError::internal(err.to_string()))?;
            Ok(AgentPromptDto {
                id: prompt.id.to_string(),
                system: prompt.system.to_string(),
            })
        })
        .collect::<CommandResult<Vec<_>>>()?;
    let tools = agent
        .tool_ids()
        .iter()
        .map(|id| {
            let tool = agent.tool(id).map_err(|err| CommandError::internal(err.to_string()))?;
            Ok(AgentToolDto {
                id: tool.id.to_string(),
                description: tool.description.to_string(),
            })
        })
        .collect::<CommandResult<Vec<_>>>()?;
    let (attempt, attempt_note) = attempt_fields(agent.id());
    let (temperature, temperature_note) =
        temperature_fields(agent.id(), !agent.prompt_ids().is_empty());
    Ok(AgentDetailDto {
        id: agent.id().to_string(),
        display_name: agent.display_name().to_string(),
        description: agent.description().to_string(),
        prompts,
        tools,
        attempt,
        attempt_note,
        temperature,
        temperature_note,
    })
}

#[tauri::command]
pub fn agents_list() -> CommandResult<Vec<AgentSummaryDto>> {
    let attempts = snapshot_agent_attempts();
    let temperatures = snapshot_agent_temperatures();
    Ok(agent_registry::all()
        .iter()
        .map(|agent| AgentSummaryDto {
            id: agent.id.to_string(),
            display_name: agent.display_name.to_string(),
            description: agent.description.to_string(),
            prompt_count: agent.prompt_ids.len(),
            tool_count: agent.tool_ids.len(),
            attempt: attempts.get(agent.id).copied(),
            temperature: temperatures.get(agent.id).copied(),
        })
        .collect())
}

#[tauri::command]
pub fn agents_get(request: AgentGetRequest) -> CommandResult<AgentDetailDto> {
    let agent_id = request.agent_id.trim();
    if agent_id.is_empty() {
        return Err(CommandError::invalid_input("agent_id is required"));
    }
    detail_for(agent_id)
}

#[tauri::command]
pub async fn agents_set_attempt(
    state: State<'_, AppState>,
    request: AgentSetAttemptRequest,
) -> CommandResult<AgentDetailDto> {
    let agent_id = request.agent_id.trim();
    if agent_id.is_empty() {
        return Err(CommandError::invalid_input("agent_id is required"));
    }
    if agent_registry::load(agent_id).is_err() {
        return Err(CommandError::invalid_input(format!("unknown agent: {agent_id}")));
    }
    persist_agent_attempt(state.database(), agent_id, request.attempt).await?;
    detail_for(agent_id)
}

#[tauri::command]
pub async fn agents_set_temperature(
    state: State<'_, AppState>,
    request: AgentSetTemperatureRequest,
) -> CommandResult<AgentDetailDto> {
    let agent_id = request.agent_id.trim();
    if agent_id.is_empty() {
        return Err(CommandError::invalid_input("agent_id is required"));
    }
    let agent = agent_registry::load(agent_id)
        .map_err(|_| CommandError::invalid_input(format!("unknown agent: {agent_id}")))?;
    if agent.prompt_ids().is_empty() {
        return Err(CommandError::invalid_input(
            "this agent does not call an LLM",
        ));
    }
    persist_agent_temperature(state.database(), agent_id, request.temperature).await?;
    detail_for(agent_id)
}
