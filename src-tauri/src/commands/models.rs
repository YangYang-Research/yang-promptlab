//! Local model vault commands — browse, install, remove, verify, inference test.

use std::sync::{Arc, Mutex, OnceLock};

use aisec_auth::SecretStore;
use aisec_core::AisecError;
use aisec_models::{
    DownloadManager, DownloadProgress, DownloadStatus, LocalModelManager, ModelCatalogEntry,
    ModelEntry, ModelFormat, ModelProvider, ModelSource, VerificationResult, remote_entry_id,
};
use aisec_runtime::RuntimeError;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tauri::async_runtime::Mutex as AsyncMutex;
use tauri::State;

use crate::inference_host::{test_inference_for_entry, test_remote_connectivity_only};
use crate::inference_settings::{
    apply_third_party_health_check, format_health_check_timestamp,
};
use aisec_inference::config::InferenceMode;
use aisec_inference::InferenceRuntimeManager;
use crate::error::{CommandError, CommandResult};
use crate::state::AppState;
use crate::third_party_credentials::{
    apply_model_connectivity_metadata, copy_credential_metadata, credential_id_from_metadata,
    has_new_credential_input, open_model_credential_vault, persist_third_party_credentials,
    resolve_third_party_credentials, validate_metadata_credentials, ThirdPartyCredentialFields,
    API_KEY_CREDENTIAL_ID, API_KEY_ENV, AWS_SECRET_CREDENTIAL_ID, AWS_SESSION_CREDENTIAL_ID,
    LAST_CONNECTIVITY_OK,
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
    #[serde(default)]
    pub existing_model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThirdPartyModelEditDto {
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
    pub region: Option<String>,
    pub api_key_env: Option<String>,
    pub api_key_configured: bool,
    pub aws_secret_access_key_configured: bool,
    pub aws_session_token_configured: bool,
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

fn third_party_edit_dto_from_entry(entry: &ModelEntry) -> Result<ThirdPartyModelEditDto, CommandError> {
    match &entry.source {
        ModelSource::Remote {
            provider,
            model,
            base_url,
            region,
        } => Ok(ThirdPartyModelEditDto {
            provider: provider.clone(),
            model: model.clone(),
            base_url: base_url.clone(),
            region: region.clone(),
            api_key_env: api_key_env_from_metadata(&entry.metadata),
            api_key_configured: credential_id_from_metadata(&entry.metadata, API_KEY_CREDENTIAL_ID)
                .is_some(),
            aws_secret_access_key_configured: credential_id_from_metadata(
                &entry.metadata,
                AWS_SECRET_CREDENTIAL_ID,
            )
            .is_some(),
            aws_session_token_configured: credential_id_from_metadata(
                &entry.metadata,
                AWS_SESSION_CREDENTIAL_ID,
            )
            .is_some(),
        }),
        _ => Err(CommandError::invalid_input(
            "edit form only applies to third-party models",
        )),
    }
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
            existing_model_id: None,
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
    state: &AppState,
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
    let vault = open_model_credential_vault(state.data_dir())?;
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

    let entry = staging_remote_entry(&request, metadata.as_ref())?;
    let remote = InferenceRuntimeManager::remote_settings_from_entry(
        &entry,
        credentials.api_key,
        Some(credentials.aws_secret_access_key).filter(|s| !s.trim().is_empty()),
        Some(credentials.aws_session_token).filter(|s| !s.trim().is_empty()),
    )
    .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    let result = test_remote_connectivity_only(&entry, remote).await?;

    Ok(ThirdPartyModelConnectivityResultDto {
        ok: result.ok,
        provider: result.provider,
        model: result.model,
        latency_ms: result.latency_ms,
        message: result.message,
        sample_response: result.sample_response,
    })
}

fn staging_remote_entry(
    request: &ThirdPartyModelSaveRequest,
    metadata: Option<&serde_json::Value>,
) -> Result<ModelEntry, CommandError> {
    let provider = request.provider.trim();
    let model = request.model.trim();
    let now = OffsetDateTime::now_utc();
    Ok(ModelEntry {
        id: remote_entry_id(provider, model),
        name: model.to_string(),
        format: ModelFormat::Api,
        provider: ModelProvider::Remote,
        version: String::new(),
        capabilities: aisec_models::ModelCapabilities {
            chat: true,
            completion: true,
            embeddings: false,
        },
        source: ModelSource::Remote {
            provider: provider.to_string(),
            model: model.to_string(),
            base_url: request.base_url.clone(),
            region: request.region.clone(),
        },
        file_path: std::path::PathBuf::new(),
        size_bytes: None,
        checksum_sha256: None,
        verified: false,
        created_at: now,
        updated_at: now,
        metadata: metadata.cloned().unwrap_or_else(|| {
            serde_json::json!({ "remoteProvider": provider })
        }),
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

fn registry_verified_status(entry: &ModelEntry) -> bool {
    if entry.provider == ModelProvider::Remote {
        return entry.metadata
            .get(LAST_CONNECTIVITY_OK)
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
    }
    entry.verified
}

pub(crate) fn entry_to_dto(entry: &ModelEntry, vault: &std::path::Path) -> ModelEntryDto {
    let size_gb = entry
        .size_bytes
        .map(|b| (b as f64) / (1024.0 * 1024.0 * 1024.0))
        .unwrap_or(0.0);
    let verified = registry_verified_status(entry);
    ModelEntryDto {
        id: entry.id.clone(),
        name: entry.name.clone(),
        provider: entry.display_provider(),
        version: entry.version.clone(),
        format: entry.format.as_str().into(),
        size_bytes: entry.size_bytes,
        size_gb,
        verified,
        path: aisec_models::ModelRegistry::display_uri(vault, &entry.file_path),
        sha256: entry.checksum_sha256.clone(),
        capabilities: ModelCapabilitiesDto {
            chat: entry.capabilities.chat,
            completion: entry.capabilities.completion,
            embeddings: entry.capabilities.embeddings,
        },
        status: if verified {
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
        provider: if entry.provider_label.trim().is_empty() {
            entry.provider.as_str().into()
        } else {
            entry.provider_label.clone()
        },
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

    let vault = manager.vault_path().to_path_buf();
    Ok(entry.map(|item| entry_to_dto(&item, &vault)))
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
    let vault = manager.vault_path().to_path_buf();
    Ok(manager
        .list_models()
        .into_iter()
        .map(|entry| entry_to_dto(entry, &vault))
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
    let vault = manager.vault_path().to_path_buf();
    Ok(entry_to_dto(&entry, &vault))
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
    let provider = request.provider.trim();
    let model = request.model.trim();
    let new_id = remote_entry_id(provider, model);
    let existing_id = request
        .existing_model_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let mut manager = state.model_manager().lock().await;
    let preserved_metadata = existing_id
        .and_then(|id| manager.get_model(id).map(|entry| entry.metadata.clone()))
        .or_else(|| manager.get_model(&new_id).map(|entry| entry.metadata.clone()));

    let entry = manager
        .register_third_party(
            provider,
            model,
            request.base_url.clone().filter(|value| !value.trim().is_empty()),
            request.region.clone().filter(|value| !value.trim().is_empty()),
        )
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;

    if let Some(old_id) = existing_id {
        if old_id != entry.id {
            manager
                .remove_model(old_id)
                .await
                .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
        }
    }

    if let Ok(secrets) = SecretStore::new() {
        let vault = open_model_credential_vault(state.data_dir())?;
        let creds = credential_fields_from_request(&request);
        let mut metadata = serde_json::json!({ "remoteProvider": provider });
        let credential_input_changed = has_new_credential_input(&creds);

        if credential_input_changed {
            persist_third_party_credentials(&mut metadata, &creds, &vault)?;
        } else {
            let source_metadata = preserved_metadata.as_ref().unwrap_or(&entry.metadata);
            copy_credential_metadata(source_metadata, &mut metadata);
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

        let is_new_model = existing_id.is_none();
        let renamed = existing_id.is_some_and(|old_id| old_id != entry.id);
        if is_new_model || renamed || credential_input_changed {
            manager
                .set_model_verified(&entry.id, false)
                .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
        }
    } else {
        return Err(CommandError::invalid_input(
            "secure storage is unavailable — cannot save third-party credentials",
        ));
    }

    let saved = manager
        .get_model(&entry.id)
        .ok_or_else(|| CommandError::invalid_input(format!("model not found: {}", entry.id)))?
        .clone();
    let vault = manager.vault_path().to_path_buf();

    Ok(entry_to_dto(&saved, &vault))
}

#[tauri::command]
pub async fn models_third_party_edit_form(
    state: State<'_, AppState>,
    model_id: String,
) -> CommandResult<ThirdPartyModelEditDto> {
    let manager = state.model_manager().lock().await;
    let entry = manager
        .get_model(&model_id)
        .ok_or_else(|| CommandError::invalid_input(format!("model not found: {model_id}")))?;
    third_party_edit_dto_from_entry(entry)
}

#[tauri::command]
pub async fn models_test_third_party(
    state: State<'_, AppState>,
    request: ThirdPartyModelSaveRequest,
) -> CommandResult<ThirdPartyModelConnectivityResultDto> {
    let model_id = remote_entry_id(request.provider.trim(), request.model.trim());
    let metadata = {
        let manager = state.model_manager().lock().await;
        manager
            .get_model(&model_id)
            .map(|entry| entry.metadata.clone())
    };
    let result = run_third_party_connectivity_test(state.inner(), request, metadata.clone()).await?;
    if metadata.is_some() {
        persist_third_party_model_connectivity(
            state.inner(),
            &model_id,
            result.ok,
            result.latency_ms,
        )
        .await?;
    }
    Ok(result)
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
    let result = run_third_party_connectivity_test(state, request, Some(metadata)).await?;
    persist_third_party_model_connectivity(state, model_id, result.ok, result.latency_ms).await?;
    Ok(result)
}

async fn persist_third_party_model_connectivity(
    state: &AppState,
    model_id: &str,
    ok: bool,
    latency_ms: u64,
) -> CommandResult<()> {
    let checked_at = format_health_check_timestamp(OffsetDateTime::now_utc());

    {
        let mut manager = state.model_manager().lock().await;
        let entry = manager
            .get_model(model_id)
            .ok_or_else(|| CommandError::invalid_input(format!("model not found: {model_id}")))?;
        let mut metadata = entry.metadata.clone();
        apply_model_connectivity_metadata(&mut metadata, ok, latency_ms, &checked_at);
        manager
            .update_model_metadata(model_id, metadata)
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
        manager
            .set_model_verified(model_id, ok)
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    }

    let mut inference = state.inference_manager().lock().await;
    let mut config = inference.config().clone();
    if config.initialized
        && config.mode == InferenceMode::ThirdParty
        && config.selected_model_id.as_deref() == Some(model_id)
    {
        apply_third_party_health_check(&mut config, &checked_at, ok, latency_ms);
        *inference.config_mut() = config;
        inference
            .save()
            .await
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    }

    Ok(())
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
    let vault = manager.vault_path().to_path_buf();
    Ok(entry_to_dto(&entry, &vault))
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
    let vault = manager.vault_path().to_path_buf();
    Ok(entry_to_dto(&entry, &vault))
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
    let vault = manager.vault_path().to_path_buf();
    Ok(entry_to_dto(&entry, &vault))
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

fn runtime_unavailable_error() -> CommandError {
    CommandError::invalid_input(
        "embedded libllama engine is unavailable — reinitialize the engine from AI Runtime",
    )
}

fn map_runtime_test_error(err: RuntimeError, _supervisor: &aisec_runtime::RuntimeSupervisor) -> CommandError {
    match err {
        RuntimeError::Unavailable => runtime_unavailable_error(),
        other => CommandError::from(AisecError::internal(other.to_string())),
    }
}

#[tauri::command]
pub async fn models_test_inference(
    state: State<'_, AppState>,
    model_id: String,
) -> CommandResult<ModelInferenceTestResult> {
    let entry = {
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
        entry.clone()
    };

    let file_path = entry.file_path.clone();
    let use_chat = entry.capabilities.chat;

    let need_load = {
        let runtime_mgr = state.runtime_manager().lock().await;
        !runtime_mgr.is_same_model_loaded_at(&file_path).await
    };

    if need_load {
        let mut runtime_mgr = state.runtime_manager().lock().await;
        if !runtime_mgr.supervisor().runtime_available() {
            return Err(runtime_unavailable_error());
        }
        crate::commands::runtime::load_model_with_loading_cache(
            state.inner(),
            &mut runtime_mgr,
            &file_path,
            &model_id,
        )
        .await
        .map_err(|err| map_runtime_test_error(err, runtime_mgr.supervisor()))?;
        runtime_mgr.sync_lifecycle_from_supervisor();
    }

    let mut runtime_mgr = state.runtime_manager().lock().await;
    if !runtime_mgr.supervisor().runtime_available() {
        return Err(runtime_unavailable_error());
    }

    let result = test_inference_for_entry(
        state.data_dir(),
        &entry,
        state.model_provider().clone(),
        &mut runtime_mgr,
    )
    .await?;

    Ok(ModelInferenceTestResult {
        ok: result.ok,
        mode: if use_chat {
            "chat".into()
        } else {
            "completion".into()
        },
        sample: result.sample_response.unwrap_or_default(),
        message: result.message,
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
