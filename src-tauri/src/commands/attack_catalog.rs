//! Attack technique catalog IPC — list / edit / reset default prompts.

use serde::{Deserialize, Serialize};
use tauri::State;

use aisec_storage::{AttackCatalogRepository, AttackCatalogTechnique, UpdateAttackCatalogTechnique};

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttackCatalogTechniqueDto {
    pub id: String,
    pub category_id: String,
    pub name: String,
    pub description: Option<String>,
    pub content: String,
    pub default_content: String,
    pub tags: Vec<String>,
    pub surface: Option<String>,
    pub owasp: Option<String>,
    pub enabled: bool,
    pub user_modified: bool,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttackCatalogCategoryDto {
    pub id: String,
    pub label: String,
    pub technique_count: usize,
    pub enabled_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAttackCatalogTechniqueRequest {
    pub content: Option<String>,
    pub enabled: Option<bool>,
    pub name: Option<String>,
    pub description: Option<String>,
}

fn category_label(id: &str) -> String {
    match id {
        "prompt_injection" => "Prompt Injection".into(),
        "system_prompt_extraction" => "System Prompt Extraction".into(),
        "jailbreak" => "Jailbreak".into(),
        "rag_leakage" => "RAG Leakage".into(),
        "memory_poisoning" => "Memory Poisoning".into(),
        "cross_user_leakage" => "Cross User Leakage".into(),
        "agent_goal_hijacking" => "Agent Goal Hijacking".into(),
        "tool_abuse" => "Tool Abuse".into(),
        "mcp_abuse" => "MCP Abuse".into(),
        "encoding" => "Encoding".into(),
        other => other
            .split('_')
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn to_dto(row: AttackCatalogTechnique) -> AttackCatalogTechniqueDto {
    let tags: Vec<String> = serde_json::from_str(&row.tags_json).unwrap_or_default();
    AttackCatalogTechniqueDto {
        id: row.id,
        category_id: row.category_id,
        name: row.name,
        description: row.description,
        content: row.content,
        default_content: row.default_content,
        tags,
        surface: row.surface,
        owasp: row.owasp,
        enabled: row.enabled,
        user_modified: row.user_modified,
        sort_order: row.sort_order,
    }
}

pub async fn attack_catalog_list_op(
    state: &AppState,
) -> CommandResult<Vec<AttackCatalogTechniqueDto>> {
    let rows = state
        .database()
        .repositories()
        .attack_catalog()
        .list()
        .await
        .map_err(CommandError::from)?;
    Ok(rows.into_iter().map(to_dto).collect())
}

pub async fn attack_catalog_categories_op(
    state: &AppState,
) -> CommandResult<Vec<AttackCatalogCategoryDto>> {
    let rows = attack_catalog_list_op(state).await?;
    let mut by_cat: std::collections::BTreeMap<String, (usize, usize)> =
        std::collections::BTreeMap::new();
    for row in &rows {
        let entry = by_cat.entry(row.category_id.clone()).or_insert((0, 0));
        entry.0 += 1;
        if row.enabled {
            entry.1 += 1;
        }
    }
    Ok(by_cat
        .into_iter()
        .map(|(id, (technique_count, enabled_count))| AttackCatalogCategoryDto {
            label: category_label(&id),
            id,
            technique_count,
            enabled_count,
        })
        .collect())
}

pub async fn attack_catalog_update_op(
    state: &AppState,
    id: String,
    request: UpdateAttackCatalogTechniqueRequest,
) -> CommandResult<AttackCatalogTechniqueDto> {
    if request.content.is_none()
        && request.enabled.is_none()
        && request.name.is_none()
        && request.description.is_none()
    {
        return Err(CommandError::invalid_input("no fields to update"));
    }
    let updated = state
        .database()
        .repositories()
        .attack_catalog()
        .update(
            &id,
            UpdateAttackCatalogTechnique {
                name: request.name,
                description: request.description,
                content: request.content,
                enabled: request.enabled,
                ..Default::default()
            },
        )
        .await
        .map_err(CommandError::from)?;
    Ok(to_dto(updated))
}

pub async fn attack_catalog_reset_op(
    state: &AppState,
    id: String,
) -> CommandResult<AttackCatalogTechniqueDto> {
    let updated = state
        .database()
        .repositories()
        .attack_catalog()
        .reset_content(&id)
        .await
        .map_err(CommandError::from)?;
    Ok(to_dto(updated))
}

#[tauri::command]
pub async fn attack_catalog_list(
    state: State<'_, AppState>,
) -> CommandResult<Vec<AttackCatalogTechniqueDto>> {
    attack_catalog_list_op(&state).await
}

#[tauri::command]
pub async fn attack_catalog_categories(
    state: State<'_, AppState>,
) -> CommandResult<Vec<AttackCatalogCategoryDto>> {
    attack_catalog_categories_op(&state).await
}

#[tauri::command]
pub async fn attack_catalog_update(
    state: State<'_, AppState>,
    id: String,
    request: UpdateAttackCatalogTechniqueRequest,
) -> CommandResult<AttackCatalogTechniqueDto> {
    attack_catalog_update_op(&state, id, request).await
}

#[tauri::command]
pub async fn attack_catalog_reset(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<AttackCatalogTechniqueDto> {
    attack_catalog_reset_op(&state, id).await
}
