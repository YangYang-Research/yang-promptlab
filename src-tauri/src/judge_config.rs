use std::path::{Path, PathBuf};

use aisec_core::AisecError;
use aisec_judge::{JudgeMode, JudgeProviderConfig, JudgeRuntimeContext, LocalProvider};
use aisec_models::{LocalModelManager, ModelSource};
use aisec_runtime::{RuntimeSupervisor, SharedModelProvider};
use tokio::fs;

use crate::error::{CommandError, CommandResult};

pub fn judge_config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("judge_config.json")
}

pub fn models_vault_path(data_dir: &Path) -> PathBuf {
    data_dir.join("models")
}

pub async fn load_judge_config(data_dir: &Path) -> CommandResult<JudgeProviderConfig> {
    let path = judge_config_path(data_dir);
    if !path.exists() {
        return Ok(JudgeProviderConfig::default());
    }
    let raw = fs::read_to_string(&path)
        .await
        .map_err(|e| CommandError::from(AisecError::internal(format!("read judge config: {e}"))))?;
    serde_json::from_str(&raw)
        .map_err(|e| CommandError::invalid_input(format!("invalid judge config: {e}")))
}

pub async fn save_judge_config(
    data_dir: &Path,
    config: &JudgeProviderConfig,
) -> CommandResult<JudgeProviderConfig> {
    let path = judge_config_path(data_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| CommandError::from(AisecError::internal(format!("create config dir: {e}"))))?;
    }
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| CommandError::from(AisecError::internal(format!("serialize judge config: {e}"))))?;
    fs::write(&path, json)
        .await
        .map_err(|e| CommandError::from(AisecError::internal(format!("write judge config: {e}"))))?;
    Ok(config.clone())
}

/// Resolve vault model id into local provider settings before building the judge engine.
pub fn resolve_judge_local_settings(
    config: &mut JudgeProviderConfig,
    manager: &LocalModelManager,
) {
    let Some(vault_id) = config.local.vault_model_id.clone() else {
        return;
    };
    let Some(entry) = manager.get_model(&vault_id) else {
        return;
    };

    match &entry.source {
        ModelSource::Ollama { model, base_url } => {
            config.local.provider = LocalProvider::Ollama;
            config.local.model = model.clone();
            config.local.base_url = base_url.clone();
            config.local.model_path = None;
        }
        _ => {
            config.local.provider = LocalProvider::LlamaCpp;
            config.local.model = entry.name.clone();
            config.local.model_path = Some(entry.file_path.clone());
        }
    }
}

/// Prepare runtime bridge context for local judge modes.
pub async fn prepare_judge_runtime_context(
    config: &mut JudgeProviderConfig,
    manager: &LocalModelManager,
    model_provider: SharedModelProvider,
    runtime_supervisor: &mut RuntimeSupervisor,
) -> CommandResult<Option<JudgeRuntimeContext>> {
    resolve_judge_local_settings(config, manager);

    match config.mode {
        JudgeMode::LocalLlm | JudgeMode::Consensus => {
            let vault_id = config.local.vault_model_id.clone().ok_or_else(|| {
                CommandError::invalid_input(
                    "select an active vault model on the Models page for local judge modes",
                )
            })?;

            if config.local.provider == LocalProvider::Ollama {
                runtime_supervisor
                    .ensure_running()
                    .await
                    .map_err(|err| CommandError::from(AisecError::internal(err.to_string())))?;
                config.local.base_url = runtime_supervisor.base_url().to_string();
            } else if config.local.provider == LocalProvider::LlamaCpp {
                runtime_supervisor
                    .ensure_running()
                    .await
                    .map_err(|err| CommandError::from(AisecError::internal(err.to_string())))?;
                if let Some(entry) = manager.get_model(&vault_id) {
                    if entry.file_path.exists() {
                        runtime_supervisor
                            .ensure_model_loaded(&entry.file_path)
                            .await
                            .map_err(|err| {
                                CommandError::from(AisecError::internal(err.to_string()))
                            })?;
                    }
                }
                config.local.base_url = runtime_supervisor.base_url().to_string();
            }

            Ok(Some(JudgeRuntimeContext::new(model_provider, vault_id)))
        }
        _ => Ok(None),
    }
}

pub async fn build_configured_judge_engine(
    data_dir: &Path,
    manager: &LocalModelManager,
    model_provider: SharedModelProvider,
    runtime_supervisor: &mut RuntimeSupervisor,
) -> CommandResult<aisec_judge::JudgeEngine> {
    let mut config = load_judge_config(data_dir).await?;
    let runtime =
        prepare_judge_runtime_context(&mut config, manager, model_provider, runtime_supervisor)
            .await?;
    aisec_judge::build_judge_engine(&config, runtime)
        .await
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))
}

pub fn open_model_manager(
    data_dir: &Path,
    catalog: aisec_models::BuiltinCatalog,
) -> CommandResult<LocalModelManager> {
    LocalModelManager::new(models_vault_path(data_dir))
        .map(|mgr| mgr.with_catalog(catalog))
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))
}
