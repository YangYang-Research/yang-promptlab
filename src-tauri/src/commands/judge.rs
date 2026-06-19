//! Judge configuration and connectivity commands.

use aisec_core::AisecError;
use aisec_judge::{
    test_connectivity, test_model, JudgeMode, JudgeProviderConfig, LocalProvider, RemoteProvider,
    VulnerabilityCategory,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::judge_config::{load_judge_config, prepare_judge_runtime_context, resolve_judge_config_secrets, save_judge_config};
use crate::state::AppState;
use aisec_auth::SecretStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgeConfigDto {
    pub mode: String,
    pub local_provider: String,
    pub local_base_url: String,
    pub local_model: String,
    pub local_model_path: Option<String>,
    pub local_vault_model_id: Option<String>,
    pub local_llama_binary: String,
    pub local_llama_port: u16,
    pub remote_provider: String,
    pub remote_base_url: Option<String>,
    pub remote_model: String,
    pub remote_api_key: String,
    pub remote_api_key_env: Option<String>,
    #[serde(default)]
    pub remote_api_key_configured: bool,
    #[serde(default)]
    pub remote_aws_secret_access_key: String,
    #[serde(default)]
    pub remote_aws_secret_access_key_configured: bool,
    #[serde(default)]
    pub remote_aws_region: Option<String>,
    #[serde(default)]
    pub remote_aws_session_token: String,
    #[serde(default)]
    pub remote_aws_session_token_configured: bool,
    pub consensus_threshold: f32,
    pub min_confidence: f32,
    pub llm_max_tokens: u32,
    pub llm_temperature: f32,
    pub categories: Vec<String>,
}

impl From<JudgeProviderConfig> for JudgeConfigDto {
    fn from(config: JudgeProviderConfig) -> Self {
        Self {
            mode: config.mode.as_str().into(),
            local_provider: match config.local.provider {
                LocalProvider::Ollama => "ollama",
                LocalProvider::LlamaCpp => "llama_cpp",
            }
            .into(),
            local_base_url: config.local.base_url,
            local_model: config.local.model,
            local_model_path: config
                .local
                .model_path
                .map(|p| p.to_string_lossy().into_owned()),
            local_vault_model_id: config.local.vault_model_id.clone(),
            local_llama_binary: config.local.llama_binary,
            local_llama_port: config.local.llama_port,
            remote_provider: match config.remote.provider {
                RemoteProvider::OpenAi => "openai",
                RemoteProvider::Anthropic => "anthropic",
                RemoteProvider::Gemini => "gemini",
                RemoteProvider::OpenRouter => "openrouter",
                RemoteProvider::Azure => "azure",
                RemoteProvider::Bedrock => "bedrock",
            }
            .into(),
            remote_base_url: config.remote.base_url,
            remote_model: config.remote.model,
            remote_api_key: config.remote.api_key.clone(),
            remote_api_key_env: config.remote.api_key_env.clone(),
            remote_api_key_configured: config.remote.api_key_credential_id.is_some()
                || !config.remote.api_key.trim().is_empty(),
            remote_aws_secret_access_key: config.remote.aws_secret_access_key.clone(),
            remote_aws_secret_access_key_configured: config
                .remote
                .aws_secret_access_key_credential_id
                .is_some()
                || !config.remote.aws_secret_access_key.trim().is_empty(),
            remote_aws_region: config.remote.aws_region.clone(),
            remote_aws_session_token: config.remote.aws_session_token.clone(),
            remote_aws_session_token_configured: config
                .remote
                .aws_session_token_credential_id
                .is_some()
                || !config.remote.aws_session_token.trim().is_empty(),
            consensus_threshold: config.consensus_threshold,
            min_confidence: config.min_confidence,
            llm_max_tokens: config.llm_max_tokens,
            llm_temperature: config.llm_temperature,
            categories: VulnerabilityCategory::all()
                .iter()
                .map(|c| c.as_str().to_string())
                .collect(),
        }
    }
}

fn parse_mode(value: &str) -> Result<JudgeMode, CommandError> {
    JudgeMode::parse(value).ok_or_else(|| {
        CommandError::invalid_input(format!("unsupported judge mode: {value}"))
    })
}

fn parse_local_provider(value: &str) -> Result<LocalProvider, CommandError> {
    match value {
        "ollama" => Ok(LocalProvider::Ollama),
        "llama_cpp" | "llama-cpp" => Ok(LocalProvider::LlamaCpp),
        other => Err(CommandError::invalid_input(format!(
            "unsupported local provider: {other}"
        ))),
    }
}

fn parse_remote_provider(value: &str) -> Result<RemoteProvider, CommandError> {
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

fn dto_to_config(dto: JudgeConfigDto) -> Result<JudgeProviderConfig, CommandError> {
    Ok(JudgeProviderConfig {
        mode: parse_mode(&dto.mode)?,
        local: aisec_judge::LocalProviderSettings {
            provider: parse_local_provider(&dto.local_provider)?,
            base_url: dto.local_base_url,
            model: dto.local_model,
            model_path: dto.local_model_path.map(std::path::PathBuf::from),
            vault_model_id: dto.local_vault_model_id,
            llama_binary: dto.local_llama_binary,
            llama_port: dto.local_llama_port,
        },
        remote: aisec_judge::RemoteProviderSettings {
            provider: parse_remote_provider(&dto.remote_provider)?,
            base_url: dto.remote_base_url,
            model: dto.remote_model,
            api_key: dto.remote_api_key,
            api_key_credential_id: None,
            api_key_env: dto.remote_api_key_env,
            aws_secret_access_key: dto.remote_aws_secret_access_key,
            aws_secret_access_key_credential_id: None,
            aws_region: dto.remote_aws_region,
            aws_session_token: dto.remote_aws_session_token,
            aws_session_token_credential_id: None,
        },
        consensus_threshold: dto.consensus_threshold,
        min_confidence: dto.min_confidence,
        llm_max_tokens: dto.llm_max_tokens,
        llm_temperature: dto.llm_temperature,
    })
}

#[tauri::command]
pub async fn judge_config_get(state: State<'_, AppState>) -> CommandResult<JudgeConfigDto> {
    let config = load_judge_config(state.data_dir()).await?;
    Ok(JudgeConfigDto::from(config))
}

#[tauri::command]
pub async fn judge_config_save(
    state: State<'_, AppState>,
    config: JudgeConfigDto,
) -> CommandResult<JudgeConfigDto> {
    let mut parsed = dto_to_config(config)?;
    crate::commands::security::sanitize_judge_on_save(&mut parsed)?;
    let saved = save_judge_config(state.data_dir(), &parsed).await?;
    Ok(JudgeConfigDto::from(saved))
}

#[tauri::command]
pub async fn judge_test_connectivity(
    state: State<'_, AppState>,
    config: Option<JudgeConfigDto>,
) -> CommandResult<aisec_judge::JudgeConnectivityResult> {
    let mut provider_config = if let Some(dto) = config {
        dto_to_config(dto)?
    } else {
        load_judge_config(state.data_dir()).await?
    };
    if let Ok(secrets) = SecretStore::new() {
        let _ = resolve_judge_config_secrets(&mut provider_config, &secrets);
    }
    let manager = state.model_manager().lock().await;
    let mut supervisor = state.runtime_supervisor().lock().await;
    let runtime = prepare_judge_runtime_context(
        &mut provider_config,
        &manager,
        state.model_provider().clone(),
        &mut supervisor,
    )
    .await?;
    test_connectivity(&provider_config, runtime)
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))
}

#[tauri::command]
pub async fn judge_test_model(
    state: State<'_, AppState>,
    config: Option<JudgeConfigDto>,
) -> CommandResult<aisec_judge::JudgeConnectivityResult> {
    let mut provider_config = if let Some(dto) = config {
        dto_to_config(dto)?
    } else {
        load_judge_config(state.data_dir()).await?
    };
    if let Ok(secrets) = SecretStore::new() {
        let _ = resolve_judge_config_secrets(&mut provider_config, &secrets);
    }
    let manager = state.model_manager().lock().await;
    let mut supervisor = state.runtime_supervisor().lock().await;
    let runtime = prepare_judge_runtime_context(
        &mut provider_config,
        &manager,
        state.model_provider().clone(),
        &mut supervisor,
    )
    .await?;
    test_model(&provider_config, runtime)
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))
}
