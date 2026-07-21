//! Attack technique catalog IPC — list / edit / reset default prompts.

use serde::{Deserialize, Serialize};
use tauri::State;

use aisec_inference::PromptRegistry;
use aisec_storage::{AttackCatalogRepository, AttackCatalogTechnique, UpdateAttackCatalogTechnique};

use crate::error::{CommandError, CommandResult};
use crate::inference_host::{gateway_complete, is_inference_ready};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttackCatalogGeneratePromptDto {
    pub id: String,
    pub content: String,
}

fn strip_generated_prompt(raw: &str) -> String {
    let trimmed = raw.trim();
    let without_fence = if trimmed.starts_with("```") {
        let mut lines = trimmed.lines();
        let _ = lines.next();
        let body: Vec<&str> = lines.collect();
        let mut joined = body.join("\n");
        if let Some(idx) = joined.rfind("```") {
            joined.truncate(idx);
        }
        joined.trim().to_string()
    } else {
        trimmed.to_string()
    };
    without_fence
        .trim_start_matches('"')
        .trim_end_matches('"')
        .trim()
        .to_string()
}

pub async fn attack_catalog_generate_prompt_op(
    state: &AppState,
    id: String,
) -> CommandResult<AttackCatalogGeneratePromptDto> {
    let row = state
        .database()
        .repositories()
        .attack_catalog()
        .get(&id)
        .await
        .map_err(CommandError::from)?;

    {
        let inference = state.inference_manager().lock().await;
        if !is_inference_ready(&inference) {
            return Err(CommandError::invalid_input(
                "Yazg Agent is offline. Configure and start AI Runtime so Yazg is Live before generating a prompt.",
            ));
        }
    }

    let user = PromptRegistry::attack_catalog_prompt_user(
        &row.id,
        &row.name,
        &row.category_id,
        row.owasp.as_deref().unwrap_or("n/a"),
        row.description.as_deref().unwrap_or("n/a"),
        &row.content,
    );

    let inference = state.inference_manager().lock().await;
    let manager = state.model_manager().lock().await;
    let mut runtime_mgr = state.runtime_manager().lock().await;
    let raw = gateway_complete(
        state.data_dir(),
        &inference,
        &manager,
        state.model_provider().clone(),
        &mut runtime_mgr,
        Some(PromptRegistry::attack_catalog_prompt_system()),
        &user,
        1024,
        0.35,
    )
    .await?;

    let content = strip_generated_prompt(&raw);
    if content.is_empty() {
        return Err(CommandError::invalid_input(
            "AI runtime returned an empty prompt",
        ));
    }

    Ok(AttackCatalogGeneratePromptDto {
        id: row.id,
        content,
    })
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

#[tauri::command]
pub async fn attack_catalog_generate_prompt(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<AttackCatalogGeneratePromptDto> {
    attack_catalog_generate_prompt_op(&state, id).await
}
