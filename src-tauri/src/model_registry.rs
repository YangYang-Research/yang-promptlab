//! Open the local model manager (Remote-only; no built-in GGUF catalog).

use std::path::Path;

use promptlab_core::PromptLabResult;
use promptlab_models::LocalModelManager;
use promptlab_runtime::{EmbeddedModelProvider, SharedModelProvider};
use tauri::AppHandle;
use tracing::info;

pub fn open_test_model_stack(
    data_dir: &Path,
) -> PromptLabResult<(
    std::sync::Arc<tokio::sync::Mutex<LocalModelManager>>,
    SharedModelProvider,
    promptlab_harness::HarnessFactory,
    std::sync::Arc<tauri::async_runtime::Mutex<promptlab_plugin_host::PluginManager>>,
)> {
    let manager = crate::inference_host::open_model_manager(data_dir)
        .map_err(|err| promptlab_core::PromptLabError::internal(err.to_string()))?;
    let manager = std::sync::Arc::new(tokio::sync::Mutex::new(manager));
    let provider = std::sync::Arc::new(EmbeddedModelProvider::new(manager.clone()));
    let harness_factory = promptlab_harness::HarnessFactory::new()
        .map_err(|err| promptlab_core::PromptLabError::internal(err.to_string()))?;
    let plugin_manager = std::sync::Arc::new(tauri::async_runtime::Mutex::new(
        crate::plugin_service::bootstrap_plugin_manager(data_dir).unwrap_or_else(|_| {
            promptlab_plugin_host::PluginManager::new(data_dir.join("plugins"))
                .expect("plugin manager")
        }),
    ));
    Ok((manager, provider, harness_factory, plugin_manager))
}

pub async fn open_model_manager_with_registry(
    _app: &AppHandle,
    data_dir: &Path,
) -> PromptLabResult<LocalModelManager> {
    let mut manager = crate::inference_host::open_model_manager(data_dir)
        .map_err(|err| promptlab_core::PromptLabError::internal(err.to_string()))?;
    let recovered = manager
        .recover_orphan_downloads()
        .await
        .unwrap_or_else(|err| {
            tracing::warn!(error = %err, "orphan download recovery skipped");
            0
        });
    if recovered > 0 {
        info!(recovered, "registered recovered model downloads on startup");
    }
    if let Err(err) = manager.restore_persisted_pipelines().await {
        tracing::warn!(error = %err, "pipeline restore skipped");
    }
    if let Ok(secrets) = promptlab_auth::SecretStore::new() {
        match crate::third_party_credentials::migrate_third_party_model_credentials(
            data_dir,
            &mut manager,
            &secrets,
        )
        .await
        {
            Ok(count) if count > 0 => {
                info!(count, "migrated third-party model credentials out of judge scope");
            }
            Ok(_) => {}
            Err(err) => {
                tracing::warn!(error = %err, "third-party credential migration skipped");
            }
        }
    }
    Ok(manager)
}
