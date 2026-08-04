//! AgentTrace IPC — traces for Agent Trace UI.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;
use promptlab_agenttrace::{ListTracesFilter, TraceDetail, TraceSummary};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTracesRequest {
    pub experiment: Option<String>,
    pub session_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTraceRequest {
    pub trace_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSessionResponse {
    pub deleted: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSessionsRequest {
    pub experiment: Option<String>,
    pub limit: Option<usize>,
}

#[tauri::command]
pub async fn agenttrace_list_sessions(
    state: State<'_, AppState>,
    request: ListSessionsRequest,
) -> CommandResult<Vec<promptlab_agenttrace::SessionSummary>> {
    let limit = request.limit.unwrap_or(100);
    state
        .agent_trace()
        .list_sessions(request.experiment.as_deref(), limit)
        .await
        .map_err(|err| CommandError::internal(err.to_string()))
}

#[tauri::command]
pub async fn agenttrace_list_traces(
    state: State<'_, AppState>,
    request: ListTracesRequest,
) -> CommandResult<Vec<TraceSummary>> {
    state
        .agent_trace()
        .list_traces(ListTracesFilter {
            experiment: request.experiment.or(Some("yazg".into())),
            session_id: request.session_id,
            limit: request.limit,
        })
        .await
        .map_err(|err| CommandError::internal(err.to_string()))
}

#[tauri::command]
pub async fn agenttrace_get_trace(
    state: State<'_, AppState>,
    request: GetTraceRequest,
) -> CommandResult<Option<TraceDetail>> {
    let trace_id = request.trace_id.trim();
    if trace_id.is_empty() {
        return Err(CommandError::invalid_input("trace_id is required"));
    }
    state
        .agent_trace()
        .get_trace(trace_id)
        .await
        .map_err(|err| CommandError::internal(err.to_string()))
}

#[tauri::command]
pub async fn agenttrace_delete_session(
    state: State<'_, AppState>,
    request: DeleteSessionRequest,
) -> CommandResult<DeleteSessionResponse> {
    let session_id = request.session_id.trim();
    if session_id.is_empty() {
        return Err(CommandError::invalid_input("session_id is required"));
    }
    let deleted = state
        .agent_trace()
        .delete_session(session_id)
        .await
        .map_err(|err| CommandError::internal(err.to_string()))?;
    Ok(DeleteSessionResponse { deleted })
}
