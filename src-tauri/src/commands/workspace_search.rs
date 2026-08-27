use serde::Serialize;
use tauri::State;

use promptlab_storage::{search_workspace, WorkspaceSearchHit};

use crate::error::CommandResult;
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchHitDto {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: String,
    pub to: String,
}

impl From<WorkspaceSearchHit> for WorkspaceSearchHitDto {
    fn from(hit: WorkspaceSearchHit) -> Self {
        Self {
            id: hit.id,
            kind: hit.kind,
            title: hit.title,
            subtitle: hit.subtitle,
            to: hit.to,
        }
    }
}

pub async fn workspace_search_op(state: &AppState, query: String) -> CommandResult<Vec<WorkspaceSearchHitDto>> {
    let hits = search_workspace(state.database().pool(), &query).await?;
    Ok(hits.into_iter().map(WorkspaceSearchHitDto::from).collect())
}

#[tauri::command]
pub async fn workspace_search(
    state: State<'_, AppState>,
    query: String,
) -> CommandResult<Vec<WorkspaceSearchHitDto>> {
    workspace_search_op(state.inner(), query).await
}
