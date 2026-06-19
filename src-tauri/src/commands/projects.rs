//! Project IPC commands — thin wrappers over `ProjectRepository` (SQLite).
//!
//! Each command delegates to a testable `*_op` function so integration tests
//! can exercise the same logic without a Tauri runtime.

use aisec_storage::{CreateProject, ProjectRepository};
use tauri::State;
use tracing::{debug, info, instrument};

use crate::dto::ProjectDto;
use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

fn validate_project_name(name: &str) -> CommandResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(CommandError::invalid_input("project name must not be empty"));
    }
    Ok(trimmed.to_string())
}

fn validate_project_id(id: &str) -> CommandResult<&str> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err(CommandError::invalid_input("project id must not be empty"));
    }
    Ok(trimmed)
}

#[instrument(skip(state), fields(name = %name))]
pub async fn project_create_op(
    state: &AppState,
    name: String,
    description: Option<String>,
) -> CommandResult<ProjectDto> {
    let name = validate_project_name(&name)?;
    let description = description
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty());

    let project = state
        .repositories()
        .projects()
        .create(CreateProject { name, description })
        .await
        .map_err(CommandError::from)?;
    info!(id = %project.id, "project created");
    Ok(project.into())
}

#[instrument(skip(state))]
pub async fn project_list_op(state: &AppState) -> CommandResult<Vec<ProjectDto>> {
    let projects = state
        .repositories()
        .projects()
        .list()
        .await
        .map_err(CommandError::from)?;
    debug!(count = projects.len(), "project list");
    Ok(projects.into_iter().map(ProjectDto::from).collect())
}

#[instrument(skip(state), fields(id = %id))]
pub async fn project_get_op(state: &AppState, id: String) -> CommandResult<ProjectDto> {
    let id = validate_project_id(&id)?;
    let project = state
        .repositories()
        .projects()
        .get(id)
        .await
        .map_err(CommandError::from)?;
    debug!(id = %project.id, "project get");
    Ok(project.into())
}

#[instrument(skip(state), fields(id = %id))]
pub async fn project_update_op(
    state: &AppState,
    id: String,
    name: Option<String>,
    description: Option<String>,
) -> CommandResult<ProjectDto> {
    let id = validate_project_id(&id)?;
    let name = name.map(|value| validate_project_name(&value)).transpose()?;
    let description = description.map(|value| value.trim().to_string());

    let project = state
        .repositories()
        .projects()
        .update(
            id,
            aisec_storage::UpdateProject {
                name,
                description,
            },
        )
        .await
        .map_err(CommandError::from)?;
    info!(id = %project.id, "project updated");
    Ok(project.into())
}

#[instrument(skip(state), fields(id = %id))]
pub async fn project_delete_op(state: &AppState, id: String) -> CommandResult<()> {
    let id = validate_project_id(&id)?;
    state
        .repositories()
        .projects()
        .delete(id)
        .await
        .map_err(CommandError::from)?;
    info!(%id, "project deleted");
    Ok(())
}

#[tauri::command]
pub async fn project_create(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
) -> CommandResult<ProjectDto> {
    project_create_op(state.inner(), name, description).await
}

#[tauri::command]
pub async fn project_list(state: State<'_, AppState>) -> CommandResult<Vec<ProjectDto>> {
    project_list_op(state.inner()).await
}

#[tauri::command]
pub async fn project_get(state: State<'_, AppState>, id: String) -> CommandResult<ProjectDto> {
    project_get_op(state.inner(), id).await
}

#[tauri::command]
pub async fn project_update(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    description: Option<String>,
) -> CommandResult<ProjectDto> {
    project_update_op(state.inner(), id, name, description).await
}

#[tauri::command]
pub async fn project_delete(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    project_delete_op(state.inner(), id).await
}
