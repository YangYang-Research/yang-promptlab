//! Local model vault commands — browse, install, remove, verify, inference test.

use std::sync::{Mutex, OnceLock};

use aisec_auth::SecretStore;
use aisec_core::AisecError;
use aisec_judge::{
    test_connectivity, JudgeMode, JudgeProviderConfig, RemoteProvider,
};
use aisec_models::{
    DownloadManager, DownloadProgress, DownloadStatus, LocalModelManager, ModelCatalogEntry,
    ModelEntry, ModelProvider, ModelSource, VerificationResult,
};
use aisec_runtime::{InferRequest, RuntimeError};
use serde::{Deserialize, Serialize};
use tauri::async_runtime::Mutex as AsyncMutex;
use tauri::State;

use std::sync::Arc;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;
use crate::third_party_credentials::{
    copy_credential_metadata, credential_id_from_metadata, has_new_credential_input,
    open_model_credential_vault, persist_third_party_credentials, resolve_third_party_credentials,
    validate_metadata_credentials, ThirdPartyCredentialFields, API_KEY_ENV,
};

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
pub struct ThirdPartyModelSaveRequest {
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
    pub region: Option<String>,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub aws_secret_access_key: String,
    #[serde(default)]
    pub aws_session_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThirdPartyModelConnectivityResultDto {
    pub ok: bool,
    pub provider: String,
    pub model: String,
    pub latency_ms: u64,
    pub message: String,
    pub sample_response: Option<String>,
}

fn api_key_env_from_metadata(metadata: &serde_json::Value) -> Option<String> {
    metadata
        .get("apiKeyEnv")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn third_party_request_from_entry(
    entry: &ModelEntry,
) -> Result<ThirdPartyModelSaveRequest, CommandError> {
    match &entry.source {
        ModelSource::Remote {
            provider,
            model,
            base_url,
            region,
        } => Ok(ThirdPartyModelSaveRequest {
            provider: provider.clone(),
            model: model.clone(),
            base_url: base_url.clone(),
            region: region.clone(),
            api_key: String::new(),
            api_key_env: api_key_env_from_metadata(&entry.metadata),
            aws_secret_access_key: String::new(),
            aws_session_token: String::new(),
        }),
        _ => Err(CommandError::invalid_input(
            "connection test only applies to third-party models",
        )),
    }
}

fn credential_fields_from_request(request: &ThirdPartyModelSaveRequest) -> ThirdPartyCredentialFields {
    ThirdPartyCredentialFields {
        api_key: request.api_key.clone(),
        api_key_env: request.api_key_env.clone(),
        aws_secret_access_key: request.aws_secret_access_key.clone(),
        aws_session_token: request.aws_session_token.clone(),
    }
}

fn apply_credential_fields(
    request: &mut ThirdPartyModelSaveRequest,
    credentials: &ThirdPartyCredentialFields,
) {
    request.api_key = credentials.api_key.clone();
    request.api_key_env = credentials.api_key_env.clone();
    request.aws_secret_access_key = credentials.aws_secret_access_key.clone();
    request.aws_session_token = credentials.aws_session_token.clone();
}

async fn run_third_party_connectivity_test(
    data_dir: &std::path::Path,
    mut request: ThirdPartyModelSaveRequest,
    metadata: Option<serde_json::Value>,
) -> CommandResult<ThirdPartyModelConnectivityResultDto> {
    if request.model.trim().is_empty() {
        return Err(CommandError::invalid_input("model name is required"));
    }
    if request.provider.trim().is_empty() {
        return Err(CommandError::invalid_input("provider is required"));
    }

    let mut credentials = credential_fields_from_request(&request);
    let vault = open_model_credential_vault(data_dir)?;
    let secrets = SecretStore::new().map_err(|e| {
        CommandError::invalid_input(format!("secure storage unavailable: {e}"))
    })?;
    resolve_third_party_credentials(
        &mut credentials,
        metadata.as_ref(),
        &vault,
        &secrets,
    )?;
    apply_credential_fields(&mut request, &credentials);

    let config = third_party_request_to_judge_config(&request)?;
    let result = test_connectivity(&config, None)
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;

    Ok(ThirdPartyModelConnectivityResultDto {
        ok: result.ok,
        provider: result.provider,
        model: result.model,
        latency_ms: result.latency_ms,
        message: result.message,
        sample_response: result.sample_response,
    })
}

fn parse_third_party_remote_provider(value: &str) -> Result<RemoteProvider, CommandError> {
    match value {
        "openai" => Ok(RemoteProvider::OpenAi),
        "anthropic" => Ok(RemoteProvider::Anthropic),
        "gemini" | "google" => Ok(RemoteProvider::Gemini),
        "openrouter" => Ok(RemoteProvider::OpenRouter),
        "azure" => Ok(RemoteProvider::Azure),
        "bedrock" | "aws_bedrock" => Ok(RemoteProvider::Bedrock),
        other => Err(CommandError::invalid_input(format!(
            "unsupported remote provider: {other}"
        ))),
    }
}

fn third_party_request_to_judge_config(
    request: &ThirdPartyModelSaveRequest,
) -> Result<JudgeProviderConfig, CommandError> {
    Ok(JudgeProviderConfig {
        mode: JudgeMode::RemoteLlm,
        local: aisec_judge::LocalProviderSettings::default(),
        remote: aisec_judge::RemoteProviderSettings {
            provider: parse_third_party_remote_provider(request.provider.trim())?,
            base_url: request.base_url.clone(),
            model: request.model.trim().to_string(),
            api_key: request.api_key.clone(),
            api_key_credential_id: None,
            api_key_env: request.api_key_env.clone(),
            aws_secret_access_key: request.aws_secret_access_key.clone(),
            aws_secret_access_key_credential_id: None,
            aws_region: request.region.clone(),
            aws_session_token: request.aws_session_token.clone(),
            aws_session_token_credential_id: None,
        },
        ..JudgeProviderConfig::default()
    })
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
    pub registered_count: usize,
    pub installed_local_count: usize,
    pub installed_bytes: u64,
    pub installed_gb: f64,
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

pub(crate) fn entry_to_dto(entry: &ModelEntry) -> ModelEntryDto {
    let size_gb = entry
        .size_bytes
        .map(|b| (b as f64) / (1024.0 * 1024.0 * 1024.0))
        .unwrap_or(0.0);
    ModelEntryDto {
        id: entry.id.clone(),
        name: entry.name.clone(),
        provider: entry.display_provider(),
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
        DownloadStatus::Verifying => "verifying",
        DownloadStatus::AwaitingVerify => "downloaded",
        DownloadStatus::VerifyFailed => "verify_failed",
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

async fn progress_to_dto_enriched(progress: &DownloadProgress) -> ModelDownloadProgressDto {
    let mut dto = progress_to_dto(progress);
    if progress.status == DownloadStatus::Completed
        && DownloadManager::is_post_download_awaiting_verify(&progress.destination).await
    {
        dto.status = "downloaded".to_string();
    }
    dto
}

fn finalize_lock() -> &'static AsyncMutex<()> {
    static LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| AsyncMutex::new(()))
}

static PENDING_INSTALL: Mutex<Option<ModelEntryDto>> = Mutex::new(None);

fn take_pending_install() -> Option<ModelEntryDto> {
    PENDING_INSTALL.lock().ok()?.take()
}

fn store_pending_install(dto: ModelEntryDto) {
    if let Ok(mut guard) = PENDING_INSTALL.lock() {
        *guard = Some(dto);
    }
}

async fn run_download_finalize(
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
) -> CommandResult<Option<ModelEntryDto>> {
    let _finalize_guard = finalize_lock().lock().await;

    let plan = {
        let mut manager = model_manager.lock().await;
        manager
            .prepare_finalize()
            .await
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?
    };

    let Some(plan) = plan else {
        return Ok(None);
    };

    let expected_sha256 = plan.catalog.sha256.as_deref().filter(|s| !s.is_empty());
    if expected_sha256.is_none() {
        tracing::warn!(
            catalog_id = %plan.catalog_id,
            "registry entry has no sha256; installing without integrity verification"
        );
    }

    let verification = match aisec_models::VerificationEngine::verify_file(
        &plan.destination,
        expected_sha256,
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            let message = format!("verify error: {err}");
            let mut manager = model_manager.lock().await;
            let _ = manager
                .record_verify_error(&plan.destination, message)
                .await;
            return Ok(None);
        }
    };

    let mut manager = model_manager.lock().await;
    let entry = manager
        .complete_finalize(plan, verification)
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;

    Ok(entry.map(|item| entry_to_dto(&item)))
}

fn spawn_download_finalize(state: &AppState) {
    let model_manager = state.model_manager().clone();
    tauri::async_runtime::spawn(async move {
        match run_download_finalize(model_manager).await {
            Ok(Some(dto)) => store_pending_install(dto),
            Ok(None) => {}
            Err(err) => tracing::warn!(error = %err, "background download finalize failed"),
        }
    });
}

async fn download_status_snapshot(
    state: &AppState,
    kick_finalize: bool,
) -> CommandResult<ModelDownloadStatusDto> {
    if let Some(installed) = take_pending_install() {
        return Ok(ModelDownloadStatusDto {
            active: false,
            progress: None,
            installed: Some(installed),
        });
    }

    {
        let mut manager = state.model_manager().lock().await;
        if manager.download_status().await.is_none() {
            let _ = manager.restore_persisted_pipelines().await;
        }
    }

    if kick_finalize {
        spawn_download_finalize(state);
    }

    let manager = state.model_manager().lock().await;
    let progress = if let Some(active) = manager.download_status().await {
        Some(progress_to_dto_enriched(&active).await)
    } else if let Some(persisted) = manager.persisted_pipeline_progress().await {
        Some(progress_to_dto_enriched(&persisted).await)
    } else {
        None
    };

    Ok(ModelDownloadStatusDto {
        active: progress.is_some(),
        progress,
        installed: None,
    })
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
pub async fn models_save_third_party(
    state: State<'_, AppState>,
    request: ThirdPartyModelSaveRequest,
) -> CommandResult<ModelEntryDto> {
    if request.model.trim().is_empty() {
        return Err(CommandError::invalid_input("model name is required"));
    }
    if request.provider.trim().is_empty() {
        return Err(CommandError::invalid_input("provider is required"));
    }
    let mut manager = state.model_manager().lock().await;
    let entry = manager
        .register_third_party(
            request.provider.trim(),
            request.model.trim(),
            request.base_url.clone().filter(|value| !value.trim().is_empty()),
            request.region.clone().filter(|value| !value.trim().is_empty()),
        )
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;

    if let Ok(secrets) = SecretStore::new() {
        let vault = open_model_credential_vault(state.data_dir())?;
        let creds = credential_fields_from_request(&request);
        let mut metadata = serde_json::json!({ "remoteProvider": request.provider.trim() });

        if has_new_credential_input(&creds) {
            persist_third_party_credentials(&mut metadata, &creds, &vault)?;
        } else {
            copy_credential_metadata(&entry.metadata, &mut metadata);
            if creds.api_key_env.is_none() {
                if let Some(env) = credential_id_from_metadata(&entry.metadata, API_KEY_ENV) {
                    metadata[API_KEY_ENV] = serde_json::Value::String(env);
                }
            } else if let Some(env) = creds.api_key_env.as_ref().filter(|v| !v.trim().is_empty()) {
                metadata[API_KEY_ENV] = serde_json::Value::String(env.trim().to_string());
            }
            validate_metadata_credentials(&metadata, &vault, &secrets)?;
        }

        manager
            .update_model_metadata(&entry.id, metadata)
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    } else {
        return Err(CommandError::invalid_input(
            "secure storage is unavailable — cannot save third-party credentials",
        ));
    }

    let updated = manager
        .get_model(&entry.id)
        .ok_or_else(|| CommandError::invalid_input(format!("model not found: {}", entry.id)))?;
    Ok(entry_to_dto(updated))
}

#[tauri::command]
pub async fn models_test_third_party(
    state: State<'_, AppState>,
    request: ThirdPartyModelSaveRequest,
) -> CommandResult<ThirdPartyModelConnectivityResultDto> {
    let model_id = format!("remote-{}", request.provider.trim());
    let metadata = {
        let manager = state.model_manager().lock().await;
        manager
            .get_model(&model_id)
            .map(|entry| entry.metadata.clone())
    };
    run_third_party_connectivity_test(state.data_dir(), request, metadata).await
}

#[tauri::command]
pub async fn models_test_connection(
    state: State<'_, AppState>,
    model_id: String,
) -> CommandResult<ThirdPartyModelConnectivityResultDto> {
    test_third_party_model_connection(state.inner(), &model_id).await
}

pub(crate) async fn test_third_party_model_connection(
    state: &AppState,
    model_id: &str,
) -> CommandResult<ThirdPartyModelConnectivityResultDto> {
    let (request, metadata) = {
        let manager = state.model_manager().lock().await;
        let entry = manager
            .get_model(model_id)
            .ok_or_else(|| CommandError::invalid_input(format!("model not found: {model_id}")))?;
        if entry.provider != ModelProvider::Remote {
            return Err(CommandError::invalid_input(
                "connection test only applies to third-party models",
            ));
        }
        let metadata = entry.metadata.clone();
        let request = third_party_request_from_entry(entry)?;
        (request, metadata)
    };
    run_third_party_connectivity_test(state.data_dir(), request, Some(metadata)).await
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
    download_status_snapshot(state.inner(), true).await
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
pub async fn models_download_retry_verify(
    state: State<'_, AppState>,
    request: ModelDownloadRequest,
) -> CommandResult<ModelDownloadStatusDto> {
    let progress = {
        let mut manager = state.model_manager().lock().await;
        manager
            .begin_catalog_verify(&request.catalog_id)
            .await
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?
    };
    spawn_download_finalize(state.inner());
    Ok(ModelDownloadStatusDto {
        active: true,
        progress: Some(progress_to_dto_enriched(&progress).await),
        installed: None,
    })
}

#[tauri::command]
pub async fn models_download_cancel_verify(
    state: State<'_, AppState>,
) -> CommandResult<ModelDownloadProgressDto> {
    let mut manager = state.model_manager().lock().await;
    let progress = manager
        .cancel_catalog_verify()
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    Ok(progress_to_dto_enriched(&progress).await)
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

fn llama_server_missing_error(supervisor: &aisec_runtime::RuntimeSupervisor) -> CommandError {
    CommandError::invalid_input(format!(
        "llama-server not found at {} — build or download llama.cpp server to runtime/llama-server (see runtime/README.md) or install llama-server on PATH",
        supervisor.binary_path().display()
    ))
}

fn map_runtime_test_error(err: RuntimeError, supervisor: &aisec_runtime::RuntimeSupervisor) -> CommandError {
    match err {
        RuntimeError::Unavailable => llama_server_missing_error(supervisor),
        other => CommandError::from(AisecError::internal(other.to_string())),
    }
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

    if entry.provider == ModelProvider::Remote {
        return Err(CommandError::invalid_input(
            "use Test Connection for third-party cloud models",
        ));
    }

    if !entry.file_path.exists() {
        return Err(CommandError::invalid_input(format!(
            "model file missing: {}",
            entry.file_path.display()
        )));
    }

    let file_path = entry.file_path.clone();
    let use_chat = entry.capabilities.chat;
    drop(manager);

    let mut manager = state.runtime_manager().lock().await;
    if !manager.supervisor().binary_available() {
        return Err(llama_server_missing_error(manager.supervisor()));
    }

    manager.on_model_load_started();
    let load_result = manager
        .supervisor_mut()
        .ensure_model_loaded(&file_path)
        .await
        .map_err(|err| map_runtime_test_error(err, manager.supervisor()));
    manager.on_model_load_finished(load_result.is_ok());
    load_result?;

    let prompt = if use_chat {
        "User: Reply with exactly: AISec OK\nAssistant: ".into()
    } else {
        "Reply with exactly: AISec OK".into()
    };

    let response = manager
        .supervisor()
        .llama_runtime()
        .infer(InferRequest {
            prompt,
            max_tokens: 16,
            temperature: 0.0,
        })
        .await
        .map_err(|err| map_runtime_test_error(err, manager.supervisor()))?;

    Ok(ModelInferenceTestResult {
        ok: !response.text.is_empty(),
        mode: if use_chat {
            "chat".into()
        } else {
            "completion".into()
        },
        sample: response.text,
        message: if use_chat {
            "Chat inference succeeded".into()
        } else {
            "Completion inference succeeded".into()
        },
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
        registered_count: stats.registered_count,
        installed_local_count: stats.installed_local_count,
        installed_bytes: stats.installed_bytes,
        installed_gb: stats.installed_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
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
