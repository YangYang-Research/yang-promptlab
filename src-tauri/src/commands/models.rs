//! Model registry commands — remote providers, verify, inference test.

use std::time::Duration;

use promptlab_auth::SecretStore;
use promptlab_core::{PromptLabError, LogCategory};
use promptlab_models::{
    ModelEntry, ModelFormat, ModelProvider, ModelSource, VerificationResult, remote_entry_id,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tauri::State;

use crate::inference_host::test_remote_connectivity_only;
use crate::inference_settings::{
    apply_third_party_health_check, format_health_check_timestamp,
};
use promptlab_inference::config::InferenceMode;
use promptlab_inference::InferenceRuntimeManager;
use crate::error::{CommandError, CommandResult};
use crate::state::AppState;
use crate::third_party_credentials::{
    apply_model_connectivity_metadata, copy_credential_metadata, credential_id_from_metadata,
    has_new_credential_input, open_model_credential_vault, persist_third_party_credentials,
    resolve_third_party_credentials, validate_metadata_credentials, ThirdPartyCredentialFields,
    API_KEY_CREDENTIAL_ID, API_KEY_ENV, AWS_SECRET_CREDENTIAL_ID, AWS_SESSION_CREDENTIAL_ID,
    LAST_CONNECTIVITY_OK,
};

const MODEL_OPERATION_TIMEOUT: Duration = Duration::from_secs(45);

async fn with_model_operation_timeout<T, F>(future: F) -> CommandResult<T>
where
    F: std::future::Future<Output = CommandResult<T>>,
{
    tokio::time::timeout(MODEL_OPERATION_TIMEOUT, future)
        .await
        .map_err(|_| {
            CommandError::invalid_input("operation timed out after 45 seconds")
        })?
}

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
    /// When true, apply a prior successful Test Connection as verified on save.
    #[serde(default)]
    pub mark_verified: bool,
    #[serde(default)]
    pub test_latency_ms: Option<u64>,
    /// Sampling temperature for this model. Default 0.7.
    #[serde(default = "default_model_temperature")]
    pub temperature: f32,
    /// Context window in tokens. Default 128000.
    #[serde(default = "default_context_window")]
    pub context_window: u32,
}

fn default_model_temperature() -> f32 {
    DEFAULT_MODEL_TEMPERATURE
}

fn default_context_window() -> u32 {
    DEFAULT_CONTEXT_WINDOW
}

pub(crate) const DEFAULT_MODEL_TEMPERATURE: f32 = 0.7;
pub(crate) const DEFAULT_CONTEXT_WINDOW: u32 = 128_000;

pub(crate) fn sampling_from_metadata(metadata: &serde_json::Value) -> (f32, u32) {
    let temperature = metadata
        .get("temperature")
        .and_then(|value| value.as_f64())
        .map(|value| value as f32)
        .filter(|value| value.is_finite() && (0.0..=2.0).contains(value))
        .unwrap_or(DEFAULT_MODEL_TEMPERATURE);
    let context_window = metadata
        .get("contextWindow")
        .and_then(|value| value.as_u64())
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value >= 1)
        .unwrap_or(DEFAULT_CONTEXT_WINDOW);
    (temperature, context_window)
}

fn validate_sampling(temperature: f32, context_window: u32) -> Result<(), CommandError> {
    if !temperature.is_finite() || !(0.0..=2.0).contains(&temperature) {
        return Err(CommandError::invalid_input(
            "temperature must be between 0 and 2",
        ));
    }
    if context_window < 1 {
        return Err(CommandError::invalid_input(
            "context window size must be a positive integer",
        ));
    }
    Ok(())
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
    pub temperature: f32,
    pub context_window: u32,
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
            temperature: sampling_from_metadata(&entry.metadata).0,
            context_window: sampling_from_metadata(&entry.metadata).1,
        }),
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
            mark_verified: false,
            test_latency_ms: None,
            temperature: sampling_from_metadata(&entry.metadata).0,
            context_window: sampling_from_metadata(&entry.metadata).1,
        }),
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

fn looks_like_openrouter_model(model: &str) -> bool {
    let model = model.trim();
    model.contains('/') && !model.contains(' ')
}

fn third_party_requires_api_key(provider: &str) -> bool {
    matches!(
        provider.trim().to_ascii_lowercase().as_str(),
        "nvidia"
            | "openai"
            | "anthropic"
            | "gemini"
            | "openrouter"
            | "azure"
            | "bedrock"
            | "yangyang"
    )
}

fn normalize_third_party_provider(
    provider: &str,
    base_url: Option<&str>,
    model: Option<&str>,
) -> String {
    if base_url
        .map(|url| url.to_ascii_lowercase().contains("openrouter.ai"))
        .unwrap_or(false)
    {
        return "openrouter".to_string();
    }
    if provider.trim().eq_ignore_ascii_case("openai")
        && base_url.map(|url| url.trim().is_empty()).unwrap_or(true)
        && model.is_some_and(looks_like_openrouter_model)
    {
        return "openrouter".to_string();
    }
    provider.trim().to_string()
}

fn default_base_url_for_provider(provider: &str) -> Option<String> {
    match provider {
        "openrouter" => Some("https://openrouter.ai/api/v1".into()),
        "nvidia" => Some("https://integrate.api.nvidia.com/v1".into()),
        "yangyang" => Some("https://inference.yyng.icu/v1".into()),
        _ => None,
    }
}

fn resolve_third_party_base_url(
    provider: &str,
    base_url: Option<String>,
    model: Option<&str>,
) -> Option<String> {
    base_url
        .filter(|value| !value.trim().is_empty())
        .or_else(|| default_base_url_for_provider(provider))
        .or_else(|| {
            if model.is_some_and(looks_like_openrouter_model) {
                default_base_url_for_provider("openrouter")
            } else {
                None
            }
        })
}

fn log_model_connectivity_test(
    state: &AppState,
    provider: &str,
    model: &str,
    ok: bool,
    message: &str,
    latency_ms: u64,
) {
    let summary = if ok {
        format!("Connection successful for {provider}/{model} ({latency_ms} ms)")
    } else {
        format!("Connection failed for {provider}/{model}: {message}")
    };

    if ok {
        state.event_bus().info(
            LogCategory::Models,
            "test_connection",
            "promptlab-desktop",
            "models",
            &summary,
        );
        tracing::info!(
            provider = %provider,
            model = %model,
            latency_ms,
            "model connection test succeeded"
        );
    } else {
        state.event_bus().error(
            LogCategory::Models,
            "test_connection",
            "promptlab-desktop",
            "models",
            &summary,
        );
        tracing::warn!(
            provider = %provider,
            model = %model,
            latency_ms,
            message = %message,
            "model connection test failed"
        );
    }
}

fn log_model_connectivity_command_error(
    state: &AppState,
    model_id: Option<&str>,
    provider: Option<&str>,
    model: Option<&str>,
    error: &CommandError,
) {
    let target = match (provider, model) {
        (Some(provider), Some(model)) => format!("{provider}/{model}"),
        _ => model_id.unwrap_or("unknown").to_string(),
    };
    let summary = format!("Connection test error for {target}: {}", error.message);
    state.event_bus().error(
        LogCategory::Models,
        "test_connection",
        "promptlab-desktop",
        "models",
        &summary,
    );
    tracing::warn!(
        target = %target,
        error = %error.message,
        "model connection test command failed"
    );
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

    request.provider = normalize_third_party_provider(
        &request.provider,
        request.base_url.as_deref(),
        Some(request.model.as_str()),
    );
    request.base_url = resolve_third_party_base_url(
        &request.provider,
        request.base_url.clone(),
        Some(request.model.as_str()),
    );

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

    if third_party_requires_api_key(&request.provider) && credentials.api_key.trim().is_empty() {
        return Err(CommandError::invalid_input(
            "API key is required — re-enter it on the form, or save the model first so PromptLab can reuse the stored key",
        ));
    }

    let entry = staging_remote_entry(&request, metadata.as_ref())?;
    let remote = InferenceRuntimeManager::remote_settings_from_entry(
        &entry,
        credentials.api_key,
        Some(credentials.aws_secret_access_key).filter(|s| !s.trim().is_empty()),
        Some(credentials.aws_session_token).filter(|s| !s.trim().is_empty()),
    )
    .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))?;
    let result = test_remote_connectivity_only(&entry, remote).await?;

    log_model_connectivity_test(
        state,
        &result.provider,
        &result.model,
        result.ok,
        &result.message,
        result.latency_ms,
    );

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
        capabilities: promptlab_models::ModelCapabilities {
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
pub struct ModelVaultStatsDto {
    pub vault_path: String,
    pub registered_count: usize,
    pub installed_local_count: usize,
    pub installed_bytes: u64,
    pub installed_gb: f64,
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

pub(crate) fn entry_to_dto(entry: &ModelEntry, _vault: &std::path::Path) -> ModelEntryDto {
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
        path: entry.file_path.to_string_lossy().into_owned(),
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

pub fn models_registry_info_op(_state: &AppState) -> CommandResult<ModelRegistryInfoDto> {
    Ok(ModelRegistryInfoDto {
        entry_count: 0,
        remote_merged: false,
        remote_url: None,
        source_path: None,
        total_models: 0,
        valid_models: 0,
        invalid_models: 0,
    })
}

pub fn models_registry_diagnostics_op(_state: &AppState) -> CommandResult<ModelRegistryDiagnosticsDto> {
    Ok(ModelRegistryDiagnosticsDto {
        total_models: 0,
        valid_models: 0,
        invalid_models: 0,
        valid_ids: Vec::new(),
        invalid_ids: Vec::new(),
        issues: Vec::new(),
        healthy: true,
    })
}

#[tauri::command]
pub async fn models_registry_diagnostics(
    state: State<'_, AppState>,
) -> CommandResult<ModelRegistryDiagnosticsDto> {
    models_registry_diagnostics_op(state.inner())
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
    validate_sampling(request.temperature, request.context_window)?;
    let provider = normalize_third_party_provider(
        request.provider.trim(),
        request.base_url.as_deref(),
        Some(request.model.trim()),
    );
    let model = request.model.trim();
    let new_id = remote_entry_id(&provider, model);
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
            &provider,
            model,
            resolve_third_party_base_url(&provider, request.base_url.clone(), Some(model)),
            request.region.clone().filter(|value| !value.trim().is_empty()),
        )
        .await
        .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))?;

    if let Some(old_id) = existing_id {
        if old_id != entry.id {
            manager
                .remove_model(old_id)
                .await
                .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))?;
        }
    }

    if let Ok(secrets) = SecretStore::new() {
        let vault = open_model_credential_vault(state.data_dir())?;
        let creds = credential_fields_from_request(&request);
        let mut metadata = serde_json::json!({ "remoteProvider": provider });
        metadata["temperature"] = serde_json::json!(request.temperature);
        metadata["contextWindow"] = serde_json::json!(request.context_window);
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
            .await
            .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))?;

        let is_new_model = existing_id.is_none();
        let renamed = existing_id.is_some_and(|old_id| old_id != entry.id);
        if request.mark_verified {
            let latency_ms = request.test_latency_ms.unwrap_or(0);
            let checked_at = format_health_check_timestamp(OffsetDateTime::now_utc());
            let current = manager
                .get_model(&entry.id)
                .ok_or_else(|| {
                    CommandError::invalid_input(format!("model not found: {}", entry.id))
                })?;
            let mut metadata = current.metadata.clone();
            apply_model_connectivity_metadata(&mut metadata, true, latency_ms, &checked_at);
            manager
                .update_model_metadata(&entry.id, metadata)
                .await
                .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))?;
            manager
                .set_model_verified(&entry.id, true)
                .await
                .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))?;
        } else if is_new_model || renamed || credential_input_changed {
            manager
                .set_model_verified(&entry.id, false)
                .await
                .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))?;
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
    let provider = request.provider.trim().to_string();
    let model = request.model.trim().to_string();
    with_model_operation_timeout(async {
        let normalized_provider = normalize_third_party_provider(
            request.provider.trim(),
            request.base_url.as_deref(),
            Some(request.model.trim()),
        );
        let new_id = remote_entry_id(&normalized_provider, request.model.trim());
        let existing_id = request
            .existing_model_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let (metadata, persist_existing) = {
            let manager = state.model_manager().lock().await;
            let metadata = existing_id
                .as_deref()
                .and_then(|id| manager.get_model(id).map(|entry| entry.metadata.clone()))
                .or_else(|| manager.get_model(&new_id).map(|entry| entry.metadata.clone()));
            let persist_existing = manager.get_model(&new_id).is_some();
            (metadata, persist_existing)
        };
        let result =
            run_third_party_connectivity_test(state.inner(), request, metadata).await?;
        if persist_existing {
            persist_third_party_model_connectivity(
                state.inner(),
                &new_id,
                result.ok,
                result.latency_ms,
            )
            .await?;
        }
        Ok(result)
    })
    .await
    .map_err(|error| {
        log_model_connectivity_command_error(
            state.inner(),
            None,
            Some(&provider),
            Some(&model),
            &error,
        );
        error
    })
}

#[tauri::command]
pub async fn models_list_provider_models(
    state: State<'_, AppState>,
    request: ThirdPartyModelSaveRequest,
) -> CommandResult<crate::commands::provider_catalog::ProviderModelCatalogDto> {
    with_model_operation_timeout(list_provider_models_op(state.inner(), request)).await
}

async fn list_provider_models_op(
    state: &AppState,
    mut request: ThirdPartyModelSaveRequest,
) -> CommandResult<crate::commands::provider_catalog::ProviderModelCatalogDto> {
    use crate::commands::provider_catalog::{fetch_provider_models, ProviderModelCatalogDto};

    if request.provider.trim().is_empty() {
        return Ok(ProviderModelCatalogDto::from_models(Vec::new()));
    }

    let model_hint = request.model.trim().to_string();
    let model_hint = Some(model_hint.as_str()).filter(|value| !value.is_empty());
    request.provider = normalize_third_party_provider(
        &request.provider,
        request.base_url.as_deref(),
        model_hint,
    );
    request.base_url =
        resolve_third_party_base_url(&request.provider, request.base_url.clone(), model_hint);

    let existing_id = request
        .existing_model_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let metadata = {
        let manager = state.model_manager().lock().await;
        existing_id
            .as_deref()
            .and_then(|id| manager.get_model(id).map(|entry| entry.metadata.clone()))
    };

    let mut credentials = credential_fields_from_request(&request);
    let vault = match open_model_credential_vault(state.data_dir()) {
        Ok(vault) => vault,
        Err(_) => return Ok(ProviderModelCatalogDto::from_models(Vec::new())),
    };
    let secrets = match SecretStore::new() {
        Ok(secrets) => secrets,
        Err(_) => return Ok(ProviderModelCatalogDto::from_models(Vec::new())),
    };
    if resolve_third_party_credentials(&mut credentials, metadata.as_ref(), &vault, &secrets)
        .is_err()
    {
        return Ok(ProviderModelCatalogDto::from_models(Vec::new()));
    }
    if credentials.api_key.trim().is_empty() {
        if let Some(name) = credentials
            .api_key_env
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if let Ok(value) = std::env::var(name) {
                credentials.api_key = value;
            }
        }
    }

    let models = fetch_provider_models(
        &request.provider,
        request.base_url.as_deref(),
        request.region.as_deref(),
        &credentials.api_key,
        &credentials.aws_secret_access_key,
        &credentials.aws_session_token,
    )
    .await;
    Ok(ProviderModelCatalogDto::from_models(models))
}

#[tauri::command]
pub async fn models_test_connection(
    state: State<'_, AppState>,
    model_id: String,
) -> CommandResult<ThirdPartyModelConnectivityResultDto> {
    with_model_operation_timeout(async {
        test_third_party_model_connection(state.inner(), &model_id).await
    })
    .await
    .map_err(|error| {
        log_model_connectivity_command_error(state.inner(), Some(&model_id), None, None, &error);
        error
    })
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
    *state.runtime_model_testing_id().lock().await = Some(model_id.to_string());
    let result = run_third_party_connectivity_test(state, request, Some(metadata)).await;
    *state.runtime_model_testing_id().lock().await = None;
    let result = result?;
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
            .await
            .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))?;
        manager
            .set_model_verified(model_id, ok)
            .await
            .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))?;
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
            .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))?;
    }

    Ok(())
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
        .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))?;
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
        .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))
}

#[tauri::command]
pub async fn models_test_inference(
    state: State<'_, AppState>,
    model_id: String,
) -> CommandResult<ModelInferenceTestResult> {
    let _entry = {
        let manager = state.model_manager().lock().await;
        manager
            .get_model(&model_id)
            .ok_or_else(|| CommandError::invalid_input(format!("model not found: {model_id}")))?
            .clone()
    };

    Err(CommandError::invalid_input(
        "use Test Connection for third-party cloud models",
    ))
}

#[tauri::command]
pub async fn models_test_embeddings(
    _state: State<'_, AppState>,
    _model_id: String,
    _input: Option<String>,
) -> CommandResult<ModelInferenceTestResult> {
    Err(CommandError::invalid_input(
        "use a remote third-party provider",
    ))
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
        .map_err(|e| CommandError::from(PromptLabError::internal(e.to_string())))?;
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
    fn sampling_defaults_when_metadata_omits_them() {
        let (temperature, context_window) = sampling_from_metadata(&serde_json::json!({}));
        assert_eq!(temperature, DEFAULT_MODEL_TEMPERATURE);
        assert_eq!(context_window, DEFAULT_CONTEXT_WINDOW);
    }

    #[test]
    fn sampling_reads_saved_values() {
        let (temperature, context_window) = sampling_from_metadata(&serde_json::json!({
            "temperature": 0.2,
            "contextWindow": 32000
        }));
        assert_eq!(temperature, 0.2);
        assert_eq!(context_window, 32000);
    }
}
