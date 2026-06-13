//! Local model vault commands — browse, install, remove, verify, inference test.

use aisec_core::AisecError;
use aisec_models::{
    ModelCatalogEntry, ModelEntry, VerificationResult,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntryDto {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub version: String,
    pub format: String,
    pub size_bytes: Option<u64>,
    pub size_gb: f64,
    pub verified: bool,
    pub path: String,
    pub sha256: Option<String>,
    pub capabilities: ModelCapabilitiesDto,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCapabilitiesDto {
    pub chat: bool,
    pub completion: bool,
    pub embeddings: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogEntryDto {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub version: String,
    pub description: String,
    pub size_bytes: Option<u64>,
    pub size_gb: Option<f64>,
    pub quant: Option<String>,
    pub capabilities: ModelCapabilitiesDto,
    pub ollama_tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInstallRequest {
    pub catalog_id: String,
    pub ollama_base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInferenceTestResult {
    pub ok: bool,
    pub mode: String,
    pub sample: String,
    pub message: String,
}

fn entry_to_dto(entry: &ModelEntry) -> ModelEntryDto {
    let size_gb = entry
        .size_bytes
        .map(|b| (b as f64) / (1024.0 * 1024.0 * 1024.0))
        .unwrap_or(0.0);
    ModelEntryDto {
        id: entry.id.clone(),
        name: entry.name.clone(),
        provider: entry.provider.as_str().into(),
        version: entry.version.clone(),
        format: entry.format.as_str().into(),
        size_bytes: entry.size_bytes,
        size_gb,
        verified: entry.verified,
        path: entry.file_path.to_string_lossy().into_owned(),
        sha256: entry.checksum_sha256.clone(),
        capabilities: ModelCapabilitiesDto {
            chat: entry.capabilities.chat,
            completion: entry.capabilities.completion,
            embeddings: entry.capabilities.embeddings,
        },
        status: if entry.verified {
            "installed".into()
        } else {
            "available".into()
        },
    }
}

fn catalog_to_dto(entry: ModelCatalogEntry) -> ModelCatalogEntryDto {
    ModelCatalogEntryDto {
        id: entry.id,
        name: entry.name,
        provider: entry.provider.as_str().into(),
        version: entry.version,
        description: entry.description,
        size_bytes: entry.size_bytes,
        size_gb: entry
            .size_bytes
            .map(|b| (b as f64) / (1024.0 * 1024.0 * 1024.0)),
        quant: entry.quant,
        capabilities: ModelCapabilitiesDto {
            chat: entry.capabilities.chat,
            completion: entry.capabilities.completion,
            embeddings: entry.capabilities.embeddings,
        },
        ollama_tag: entry.ollama_tag,
    }
}

#[tauri::command]
pub async fn models_list(state: State<'_, AppState>) -> CommandResult<Vec<ModelEntryDto>> {
    let manager = state.model_manager().lock().await;
    Ok(manager
        .list_models()
        .into_iter()
        .map(entry_to_dto)
        .collect())
}

#[tauri::command]
pub async fn models_browse(state: State<'_, AppState>) -> CommandResult<Vec<ModelCatalogEntryDto>> {
    let manager = state.model_manager().lock().await;
    Ok(manager
        .browse_catalog()
        .into_iter()
        .map(catalog_to_dto)
        .collect())
}

#[tauri::command]
pub async fn models_install(
    state: State<'_, AppState>,
    request: ModelInstallRequest,
) -> CommandResult<ModelEntryDto> {
    let mut manager = state.model_manager().lock().await;
    let entry = manager
        .install_catalog(&request.catalog_id, request.ollama_base_url)
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    Ok(entry_to_dto(&entry))
}

#[tauri::command]
pub async fn models_remove(
    state: State<'_, AppState>,
    model_id: String,
) -> CommandResult<ModelEntryDto> {
    let mut manager = state.model_manager().lock().await;
    let entry = manager
        .remove_model(&model_id)
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    Ok(entry_to_dto(&entry))
}

#[tauri::command]
pub async fn models_verify(
    state: State<'_, AppState>,
    model_id: String,
) -> CommandResult<VerificationResult> {
    let mut manager = state.model_manager().lock().await;
    manager
        .verify_model(&model_id)
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))
}

#[tauri::command]
pub async fn models_test_inference(
    state: State<'_, AppState>,
    model_id: String,
) -> CommandResult<ModelInferenceTestResult> {
    let manager = state.model_manager().lock().await;
    let entry = manager
        .get_model(&model_id)
        .ok_or_else(|| CommandError::invalid_input(format!("model not found: {model_id}")))?;

    if entry.capabilities.chat {
        let sample = manager
            .test_chat(&model_id)
            .await
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
        return Ok(ModelInferenceTestResult {
            ok: !sample.is_empty(),
            mode: "chat".into(),
            sample,
            message: "Chat inference succeeded".into(),
        });
    }

    let sample = manager
        .test_inference(&model_id)
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    Ok(ModelInferenceTestResult {
        ok: !sample.is_empty(),
        mode: "completion".into(),
        sample,
        message: "Completion inference succeeded".into(),
    })
}

#[tauri::command]
pub async fn models_test_embeddings(
    state: State<'_, AppState>,
    model_id: String,
    input: Option<String>,
) -> CommandResult<ModelInferenceTestResult> {
    let manager = state.model_manager().lock().await;
    let engine = manager
        .inference_engine(&model_id)
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    let response = engine
        .embeddings(aisec_models::EmbeddingRequest {
            input: input.unwrap_or_else(|| "AISec embedding test".into()),
        })
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    Ok(ModelInferenceTestResult {
        ok: !response.vector.is_empty(),
        mode: "embeddings".into(),
        sample: format!("{} dimensions", response.dimensions),
        message: "Embedding inference succeeded".into(),
    })
}

#[tauri::command]
pub async fn models_vault_path(state: State<'_, AppState>) -> CommandResult<String> {
    Ok(state.models_dir().to_string_lossy().into_owned())
}
