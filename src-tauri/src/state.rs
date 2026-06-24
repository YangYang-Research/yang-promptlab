use std::path::{Path, PathBuf};

use std::sync::Arc;

use aisec_auth::AuthEngineConfig;
use aisec_core::LogGuard;
use aisec_harness::HarnessFactory;
use aisec_models::{BuiltinCatalogMeta, LocalModelManager};
use aisec_plugin_host::PluginManager;
use aisec_runtime::RuntimeManager;
use aisec_storage::{Database, Repositories};
use tauri::async_runtime::Mutex as AsyncMutex;

use crate::jobs::ScanJobManager;

/// Shared application state managed by Tauri and accessible from commands.
pub struct AppState {
    db: Database,
    data_dir: PathBuf,
    jobs: ScanJobManager,
    auth_engine_config: AuthEngineConfig,
    harness_factory: HarnessFactory,
    plugin_manager: Arc<AsyncMutex<PluginManager>>,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: aisec_runtime::SharedModelProvider,
    model_catalog_meta: BuiltinCatalogMeta,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    runtime_config_cache: Arc<AsyncMutex<Option<crate::commands::runtime::RuntimeConfigurationDto>>>,
    /// Model id currently loading into llama-server (survives stale config cache).
    runtime_model_loading_id: Arc<AsyncMutex<Option<String>>>,
    _log_guard: LogGuard,
}

impl AppState {
    pub fn new(
        db: Database,
        data_dir: PathBuf,
        log_guard: LogGuard,
        auth_engine_config: AuthEngineConfig,
        harness_factory: HarnessFactory,
        plugin_manager: Arc<AsyncMutex<PluginManager>>,
        runtime_manager: RuntimeManager,
        model_manager: Arc<AsyncMutex<LocalModelManager>>,
        model_provider: aisec_runtime::SharedModelProvider,
        model_catalog_meta: BuiltinCatalogMeta,
    ) -> Self {
        Self {
            db,
            data_dir,
            jobs: ScanJobManager::default(),
            auth_engine_config,
            harness_factory,
            plugin_manager,
            model_manager,
            model_provider,
            model_catalog_meta,
            runtime_manager: Arc::new(AsyncMutex::new(runtime_manager)),
            runtime_config_cache: Arc::new(AsyncMutex::new(None)),
            runtime_model_loading_id: Arc::new(AsyncMutex::new(None)),
            _log_guard: log_guard,
        }
    }

    pub fn database(&self) -> &Database {
        &self.db
    }

    pub fn jobs(&self) -> &ScanJobManager {
        &self.jobs
    }

    pub fn repositories(&self) -> Repositories {
        self.db.repositories()
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn reports_dir(&self) -> PathBuf {
        self.data_dir.join("reports")
    }

    pub fn models_dir(&self) -> PathBuf {
        self.data_dir.join("models")
    }

    pub fn model_catalog_meta(&self) -> &BuiltinCatalogMeta {
        &self.model_catalog_meta
    }

    pub fn model_manager(&self) -> &Arc<AsyncMutex<LocalModelManager>> {
        &self.model_manager
    }

    pub fn model_provider(&self) -> &aisec_runtime::SharedModelProvider {
        &self.model_provider
    }

    pub fn runtime_manager(&self) -> &Arc<AsyncMutex<RuntimeManager>> {
        &self.runtime_manager
    }

    pub fn runtime_config_cache(
        &self,
    ) -> &Arc<AsyncMutex<Option<crate::commands::runtime::RuntimeConfigurationDto>>> {
        &self.runtime_config_cache
    }

    pub fn runtime_model_loading_id(&self) -> &Arc<AsyncMutex<Option<String>>> {
        &self.runtime_model_loading_id
    }

    pub async fn ollama_base_url(&self) -> String {
        self.runtime_manager
            .lock()
            .await
            .supervisor()
            .base_url()
            .to_string()
    }

    pub fn auth_engine_config(&self) -> &AuthEngineConfig {
        &self.auth_engine_config
    }

    pub fn harness_factory(&self) -> &HarnessFactory {
        &self.harness_factory
    }

    pub fn plugin_manager(&self) -> &Arc<AsyncMutex<PluginManager>> {
        &self.plugin_manager
    }

    pub fn plugins_dir(&self) -> PathBuf {
        self.data_dir.join("plugins")
    }
}
