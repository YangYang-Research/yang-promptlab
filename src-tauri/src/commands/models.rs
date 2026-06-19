//! Local model vault commands — browse, install, remove, verify, inference test.

use aisec_core::AisecError;
use aisec_models::{
    DownloadProgress, DownloadStatus, ModelCatalogEntry, ModelEntry, VerificationResult,
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
    pub purpose: String,
    pub recommended: bool,
    pub size_bytes: Option<u64>,
    pub size_gb: Option<f64>,
    pub quant: Option<String>,
    pub capabilities: ModelCapabilitiesDto,
    pub engine: String,
    pub format: String,
    pub download_url: Option<String>,
    pub sha256: Option<String>,
    pub size_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRegistryInfoDto {
    pub entry_count: usize,
    pub remote_merged: bool,
    pub remote_url: Option<String>,
    pub source_path: Option<String>,
    pub total_models: usize,
    pub valid_models: usize,
    pub invalid_models: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryValidationIssueDto {
    pub id: String,
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRegistryDiagnosticsDto {
    pub total_models: usize,
    pub valid_models: usize,
    pub invalid_models: usize,
    pub valid_ids: Vec<String>,
    pub invalid_ids: Vec<String>,
    pub issues: Vec<RegistryValidationIssueDto>,
    pub healthy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInstallRequest {
    pub catalog_id: String,
    pub ollama_base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelImportRequest {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadRequest {
    pub catalog_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadProgressDto {
    pub catalog_id: String,
    pub status: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub remaining_bytes: Option<u64>,
    pub percent: Option<f64>,
    pub speed_bytes_per_sec: Option<f64>,
    pub eta_seconds: Option<u64>,
    pub resumed: bool,
    pub destination: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelVaultStatsDto {
    pub vault_path: String,
    pub model_count: usize,
    pub installed_bytes: u64,
    pub installed_gb: f64,
    pub disk_usage_bytes: u64,
    pub disk_usage_gb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadStatusDto {
    pub active: bool,
    pub progress: Option<ModelDownloadProgressDto>,
    pub installed: Option<ModelEntryDto>,
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

fn catalog_to_dto(entry: &ModelCatalogEntry) -> ModelCatalogEntryDto {
    ModelCatalogEntryDto {
        id: entry.id.clone(),
        name: entry.name.clone(),
        provider: entry.provider.as_str().into(),
        version: entry.version.clone(),
        description: entry.description.clone(),
        purpose: entry.purpose.clone(),
        recommended: entry.recommended,
        size_bytes: entry.size_bytes,
        size_gb: entry
            .size_bytes
            .map(|b| (b as f64) / (1024.0 * 1024.0 * 1024.0)),
        quant: entry.quant.clone(),
        capabilities: ModelCapabilitiesDto {
            chat: entry.capabilities.chat,
            completion: entry.capabilities.completion,
            embeddings: entry.capabilities.embeddings,
        },
        engine: entry.engine.clone(),
        format: entry.format.clone(),
        download_url: entry.download_url.clone(),
        sha256: entry.sha256.clone(),
        size_label: entry.size_label.clone(),
    }
}

fn status_str(status: DownloadStatus) -> &'static str {
    match status {
        DownloadStatus::Pending => "pending",
        DownloadStatus::Downloading => "downloading",
        DownloadStatus::Paused => "paused",
        DownloadStatus::Completed => "completed",
        DownloadStatus::Failed => "failed",
        DownloadStatus::Verified => "verified",
    }
}

fn progress_to_dto(progress: &DownloadProgress) -> ModelDownloadProgressDto {
    let percent = progress.total_bytes.and_then(|total| {
        if total == 0 {
            None
        } else {
            Some((progress.downloaded_bytes as f64 / total as f64) * 100.0)
        }
    });
    let remaining_bytes = progress
        .total_bytes
        .map(|total| total.saturating_sub(progress.downloaded_bytes));
    ModelDownloadProgressDto {
        catalog_id: progress.model_id.clone(),
        status: status_str(progress.status).into(),
        downloaded_bytes: progress.downloaded_bytes,
        total_bytes: progress.total_bytes,
        remaining_bytes,
        percent,
        speed_bytes_per_sec: progress.speed_bytes_per_sec,
        eta_seconds: progress.eta_seconds,
        resumed: progress.resumed,
        destination: progress.destination.to_string_lossy().into_owned(),
        error: progress.error.clone(),
    }
}

async fn sync_download_state(state: &AppState) -> CommandResult<Option<ModelEntryDto>> {
    let mut manager = state.model_manager().lock().await;
    if let Some(entry) = manager
        .finalize_active_download()
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?
    {
        return Ok(Some(entry_to_dto(&entry)));
    }
    // Note: do NOT clear failed downloads here — they must remain queryable so the
    // frontend can surface the failure reason. Completed downloads are consumed by
    // `finalize_active_download` above; failed ones are replaced on the next start.
    Ok(None)
}

#[tauri::command]
pub async fn models_list(state: State<'_, AppState>) -> CommandResult<Vec<ModelEntryDto>> {
    models_list_op(state.inner()).await
}

pub async fn models_list_op(state: &AppState) -> CommandResult<Vec<ModelEntryDto>> {
    let manager = state.model_manager().lock().await;
    Ok(manager
        .list_models()
        .into_iter()
        .map(entry_to_dto)
        .collect())
}

#[tauri::command]
pub async fn models_registry_info(state: State<'_, AppState>) -> CommandResult<ModelRegistryInfoDto> {
    models_registry_info_op(state.inner())
}

pub fn models_registry_info_op(state: &AppState) -> CommandResult<ModelRegistryInfoDto> {
    let meta = state.model_catalog_meta();
    let validation = &meta.validation;
    Ok(ModelRegistryInfoDto {
        entry_count: meta.entry_count,
        remote_merged: meta.remote_merged,
        remote_url: meta.remote_url.clone(),
        source_path: meta
            .source_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
        total_models: validation.total,
        valid_models: validation.valid,
        invalid_models: validation.invalid,
    })
}

pub fn models_registry_diagnostics_op(state: &AppState) -> CommandResult<ModelRegistryDiagnosticsDto> {
    let validation = &state.model_catalog_meta().validation;
    Ok(ModelRegistryDiagnosticsDto {
        total_models: validation.total,
        valid_models: validation.valid,
        invalid_models: validation.invalid,
        valid_ids: validation.valid_ids.clone(),
        invalid_ids: validation.invalid_ids.clone(),
        issues: validation
            .issues
            .iter()
            .map(|issue| RegistryValidationIssueDto {
                id: issue.id.clone(),
                field: issue.field.clone(),
                message: issue.message.clone(),
            })
            .collect(),
        healthy: validation.is_healthy(),
    })
}

#[tauri::command]
pub async fn models_registry_diagnostics(
    state: State<'_, AppState>,
) -> CommandResult<ModelRegistryDiagnosticsDto> {
    models_registry_diagnostics_op(state.inner())
}

#[tauri::command]
pub async fn models_browse(state: State<'_, AppState>) -> CommandResult<Vec<ModelCatalogEntryDto>> {
    models_browse_op(state.inner()).await
}

pub async fn models_browse_op(state: &AppState) -> CommandResult<Vec<ModelCatalogEntryDto>> {
    let manager = state.model_manager().lock().await;
    Ok(manager
        .browse_catalog()
        .iter()
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
        .install_catalog(&request.catalog_id, None)
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    Ok(entry_to_dto(&entry))
}

#[tauri::command]
pub async fn models_import_gguf(
    state: State<'_, AppState>,
    request: ModelImportRequest,
) -> CommandResult<ModelEntryDto> {
    let mut manager = state.model_manager().lock().await;
    let entry = manager
        .import_local(&request.name, &request.path)
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    Ok(entry_to_dto(&entry))
}

#[tauri::command]
pub async fn models_import_zip(
    state: State<'_, AppState>,
    request: ModelImportRequest,
) -> CommandResult<ModelEntryDto> {
    let mut manager = state.model_manager().lock().await;
    let entry = manager
        .import_zip_package(&request.name, &request.path)
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    Ok(entry_to_dto(&entry))
}

#[tauri::command]
pub async fn models_download_start(
    state: State<'_, AppState>,
    request: ModelDownloadRequest,
) -> CommandResult<ModelDownloadProgressDto> {
    let mut manager = state.model_manager().lock().await;
    let progress = manager
        .start_catalog_download(&request.catalog_id)
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    Ok(progress_to_dto(&progress))
}

#[tauri::command]
pub async fn models_download_status(
    state: State<'_, AppState>,
) -> CommandResult<ModelDownloadStatusDto> {
    if let Some(installed) = sync_download_state(state.inner()).await? {
        return Ok(ModelDownloadStatusDto {
            active: false,
            progress: None,
            installed: Some(installed),
        });
    }

    let manager = state.model_manager().lock().await;
    let progress = manager.download_status().await.map(|p| progress_to_dto(&p));
    Ok(ModelDownloadStatusDto {
        active: progress.is_some(),
        progress,
        installed: None,
    })
}

#[tauri::command]
pub async fn models_download_pause(
    state: State<'_, AppState>,
) -> CommandResult<ModelDownloadProgressDto> {
    let manager = state.model_manager().lock().await;
    let progress = manager
        .pause_download()
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    Ok(progress_to_dto(&progress))
}

#[tauri::command]
pub async fn models_download_resume(
    state: State<'_, AppState>,
) -> CommandResult<ModelDownloadProgressDto> {
    let manager = state.model_manager().lock().await;
    let progress = manager
        .resume_download()
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    Ok(progress_to_dto(&progress))
}

#[tauri::command]
pub async fn models_download_cancel(state: State<'_, AppState>) -> CommandResult<()> {
    let manager = state.model_manager().lock().await;
    manager
        .cancel_download()
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))
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

#[tauri::command]
pub async fn models_vault_stats(state: State<'_, AppState>) -> CommandResult<ModelVaultStatsDto> {
    models_vault_stats_op(state.inner()).await
}

pub async fn models_vault_stats_op(state: &AppState) -> CommandResult<ModelVaultStatsDto> {
    let manager = state.model_manager().lock().await;
    let stats = manager
        .vault_stats()
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    Ok(ModelVaultStatsDto {
        vault_path: stats.vault_path.to_string_lossy().into_owned(),
        model_count: stats.model_count,
        installed_bytes: stats.installed_bytes,
        installed_gb: stats.installed_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        disk_usage_bytes: stats.disk_usage_bytes,
        disk_usage_gb: stats.disk_usage_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_percent_computes() {
        let dto = progress_to_dto(&DownloadProgress {
            model_id: "hf-llama3-8b-q4".into(),
            status: DownloadStatus::Downloading,
            url: String::new(),
            destination: std::path::PathBuf::from("/tmp/model.gguf"),
            downloaded_bytes: 500,
            total_bytes: Some(1000),
            speed_bytes_per_sec: Some(100.0),
            eta_seconds: Some(5),
            resumed: false,
            updated_at: time::OffsetDateTime::now_utc(),
            error: None,
        });
        assert_eq!(dto.percent, Some(50.0));
        assert_eq!(dto.remaining_bytes, Some(500));
        assert_eq!(dto.speed_bytes_per_sec, Some(100.0));
        assert_eq!(dto.eta_seconds, Some(5));
    }
}
