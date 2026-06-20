//! Resolve and load the built-in model registry (`resources/models.json`).

use std::path::{Path, PathBuf};

use aisec_core::AisecResult;
use aisec_models::{BuiltinCatalog, BuiltinCatalogMeta, LocalModelManager};
use aisec_runtime::{EmbeddedModelProvider, SharedModelProvider};
use tauri::{AppHandle, Manager};
use tracing::info;

const REMOTE_REGISTRY_ENV: &str = "AISEC_MODEL_REGISTRY_URL";

pub fn resolve_models_json_path(app: &AppHandle) -> PathBuf {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled = resource_dir.join("resources/models.json");
        if bundled.is_file() {
            return bundled;
        }
    }

    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/models.json");
    if repo.is_file() {
        return repo;
    }

    repo
}

pub fn remote_registry_url() -> Option<String> {
    std::env::var(REMOTE_REGISTRY_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

pub async fn load_builtin_catalog(app: &AppHandle) -> AisecResult<(BuiltinCatalog, BuiltinCatalogMeta)> {
    let path = resolve_models_json_path(app);
    let remote = remote_registry_url();
    let catalog = BuiltinCatalog::load_with_optional_remote(&path, remote.as_deref())
        .await
        .map_err(|err| aisec_core::AisecError::internal(err.to_string()))?;
    let meta = catalog.meta().clone();
    info!(
        path = %path.display(),
        entries = meta.entry_count,
        valid = meta.validation.valid,
        invalid = meta.validation.invalid,
        remote_merged = meta.remote_merged,
        "loaded built-in model registry"
    );
    Ok((catalog, meta))
}

/// Load the repo registry for integration tests (offline, no remote merge).
pub fn load_repo_catalog_for_tests() -> AisecResult<(BuiltinCatalog, BuiltinCatalogMeta)> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/models.json");
    let catalog = BuiltinCatalog::load_from_path(&path)
        .map_err(|err| aisec_core::AisecError::internal(err.to_string()))?;
    let meta = catalog.meta().clone();
    Ok((catalog, meta))
}

pub fn open_test_model_stack(
    data_dir: &Path,
) -> AisecResult<(
    std::sync::Arc<tokio::sync::Mutex<LocalModelManager>>,
    SharedModelProvider,
    BuiltinCatalogMeta,
    aisec_harness::HarnessFactory,
    std::sync::Arc<tauri::async_runtime::Mutex<aisec_plugin_host::PluginManager>>,
)> {
    let (catalog, meta) = load_repo_catalog_for_tests()?;
    let manager = crate::judge_config::open_model_manager(data_dir, catalog)
        .map_err(|err| aisec_core::AisecError::internal(err.to_string()))?;
    let manager = std::sync::Arc::new(tokio::sync::Mutex::new(manager));
    let provider = std::sync::Arc::new(EmbeddedModelProvider::new(manager.clone()));
    let harness_factory = aisec_harness::HarnessFactory::new()
        .map_err(|err| aisec_core::AisecError::internal(err.to_string()))?;
    let plugin_manager = std::sync::Arc::new(tauri::async_runtime::Mutex::new(
        crate::plugin_service::bootstrap_plugin_manager(data_dir).unwrap_or_else(|_| {
            aisec_plugin_host::PluginManager::new(data_dir.join("plugins"))
                .expect("plugin manager")
        }),
    ));
    Ok((manager, provider, meta, harness_factory, plugin_manager))
}

pub async fn open_model_manager_with_registry(
    app: &AppHandle,
    data_dir: &Path,
) -> AisecResult<(LocalModelManager, BuiltinCatalogMeta)> {
    let (catalog, meta) = load_builtin_catalog(app).await?;
    let mut manager = crate::judge_config::open_model_manager(data_dir, catalog)
        .map_err(|err| aisec_core::AisecError::internal(err.to_string()))?;
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
    Ok((manager, meta))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_models_json_exists() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/models.json");
        assert!(path.is_file(), "resources/models.json must exist in repo");
    }
}
