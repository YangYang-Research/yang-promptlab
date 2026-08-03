//! Agent STM/LTM IPC (AgentCore-aligned memory browse for Agent Trace).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

use crate::agent_memory::SqliteAgentMemoryStore;
use crate::error::{CommandError, CommandResult};
use crate::state::AppState;
use promptlab_agent::{AgentMemoryStore, MemoryScopeType};

const DEFAULT_SESSION_PREFIX: &str = "yazg-chat:";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStmSessionDto {
    pub session_id: String,
    pub event_count: usize,
    pub first_at: Option<String>,
    pub last_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStmEventDto {
    pub id: String,
    pub agent_id: String,
    pub role: String,
    pub memory_key: Option<String>,
    pub content: String,
    pub content_json: Option<Value>,
    pub importance: f64,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentLtmEntryDto {
    pub id: String,
    pub agent_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub memory_key: String,
    pub content: String,
    pub importance: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListStmSessionsRequest {
    pub prefix: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListStmEventsRequest {
    pub session_id: String,
    pub agent_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListLtmRequest {
    pub agent_id: Option<String>,
    pub scope_type: String,
    pub scope_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteStmSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteStmSessionResponse {
    pub deleted: u64,
}

fn parse_scope_type(raw: &str) -> CommandResult<MemoryScopeType> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "global" => Ok(MemoryScopeType::Global),
        "project" => Ok(MemoryScopeType::Project),
        "target" => Ok(MemoryScopeType::Target),
        "scan" => Ok(MemoryScopeType::Scan),
        other => Err(CommandError::invalid_input(format!(
            "unknown scope_type '{other}' (expected global|project|target|scan)"
        ))),
    }
}

#[tauri::command]
pub async fn agent_memory_list_sessions(
    state: State<'_, AppState>,
    request: ListStmSessionsRequest,
) -> CommandResult<Vec<AgentStmSessionDto>> {
    let store = SqliteAgentMemoryStore::new(state.repositories());
    let prefix = request
        .prefix
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_SESSION_PREFIX);
    let limit = request.limit.unwrap_or(100).clamp(1, 500);
    let rows = store
        .stm_list_sessions(Some(prefix), limit)
        .await
        .map_err(CommandError::internal)?;
    Ok(rows
        .into_iter()
        .map(|row| AgentStmSessionDto {
            session_id: row.session_id,
            event_count: row.event_count,
            first_at: row.first_at,
            last_at: row.last_at,
        })
        .collect())
}

#[tauri::command]
pub async fn agent_memory_list_events(
    state: State<'_, AppState>,
    request: ListStmEventsRequest,
) -> CommandResult<Vec<AgentStmEventDto>> {
    let session_id = request.session_id.trim();
    if session_id.is_empty() {
        return Err(CommandError::invalid_input("session_id is required"));
    }
    let store = SqliteAgentMemoryStore::new(state.repositories());
    let limit = request.limit.unwrap_or(500).clamp(1, 2_000);
    let agent_id = request
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let rows = store
        .stm_list(session_id, agent_id, limit)
        .await
        .map_err(CommandError::internal)?;
    Ok(rows
        .into_iter()
        .map(|row| AgentStmEventDto {
            id: row.id,
            agent_id: row.agent_id,
            role: row.role,
            memory_key: row.memory_key,
            content: row.content,
            content_json: row.content_json,
            importance: row.importance,
            created_at: row.created_at,
        })
        .collect())
}

#[tauri::command]
pub async fn agent_memory_delete_session(
    state: State<'_, AppState>,
    request: DeleteStmSessionRequest,
) -> CommandResult<DeleteStmSessionResponse> {
    let session_id = request.session_id.trim();
    if session_id.is_empty() {
        return Err(CommandError::invalid_input("session_id is required"));
    }
    let store = SqliteAgentMemoryStore::new(state.repositories());
    let deleted = store
        .stm_delete_session(session_id)
        .await
        .map_err(CommandError::internal)?;
    Ok(DeleteStmSessionResponse { deleted })
}

#[tauri::command]
pub async fn agent_memory_list_ltm(
    state: State<'_, AppState>,
    request: ListLtmRequest,
) -> CommandResult<Vec<AgentLtmEntryDto>> {
    let scope_type = parse_scope_type(&request.scope_type)?;
    let scope_id = request.scope_id.unwrap_or_default();
    let store = SqliteAgentMemoryStore::new(state.repositories());
    let limit = request.limit.unwrap_or(64).clamp(1, 500);
    let agent_id = request
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let rows = store
        .ltm_list(agent_id, scope_type, &scope_id, limit)
        .await
        .map_err(CommandError::internal)?;
    Ok(rows
        .into_iter()
        .map(|row| AgentLtmEntryDto {
            id: row.id,
            agent_id: row.agent_id,
            scope_type: row.scope_type,
            scope_id: row.scope_id,
            memory_key: row.memory_key,
            content: row.content,
            importance: row.importance,
        })
        .collect())
}
