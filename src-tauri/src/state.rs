use std::path::{Path, PathBuf};

use std::sync::Arc;

use aisec_auth::AuthEngineConfig;
use aisec_core::LogGuard;
use aisec_models::{BuiltinCatalogMeta, LocalModelManager};
use aisec_runtime::{RuntimeSupervisor, SharedModelProvider};
use aisec_storage::{Database, Repositories};
use tauri::async_runtime::Mutex as AsyncMutex;

use crate::jobs::ScanJobManager;

/// Shared application state managed by Tauri and accessible from commands.
pub struct AppState {
    db: Database,
    data_dir: PathBuf,
    jobs: ScanJobManager,
    auth_engine_config: AuthEngineConfig,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: SharedModelProvider,
    model_catalog_meta: BuiltinCatalogMeta,
    runtime_supervisor: Arc<AsyncMutex<RuntimeSupervisor>>,
    _log_guard: LogGuard,
}

impl AppState {
    pub fn new(
        db: Database,
        data_dir: PathBuf,
        log_guard: LogGuard,
        auth_engine_config: AuthEngineConfig,
        runtime_supervisor: RuntimeSupervisor,
        model_manager: Arc<AsyncMutex<LocalModelManager>>,
        model_provider: SharedModelProvider,
        model_catalog_meta: BuiltinCatalogMeta,
    ) -> Self {
        Self {
            db,
            data_dir,
            jobs: ScanJobManager::default(),
            auth_engine_config,
            model_manager,
            model_provider,
            model_catalog_meta,
            runtime_supervisor: Arc::new(AsyncMutex::new(runtime_supervisor)),
            _log_guard: log_guard,
        }
    }

    /// The shared database handle (cheap clone of the connection pool).
    pub fn database(&self) -> &Database {
        &self.db
    }

    /// Background scan job registry.
    pub fn jobs(&self) -> &ScanJobManager {
        &self.jobs
    }

    /// Repository manager bound to the shared connection pool.
    pub fn repositories(&self) -> Repositories {
        self.db.repositories()
    }

    /// Application data directory.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Directory where generated reports are written.
    pub fn reports_dir(&self) -> PathBuf {
        self.data_dir.join("reports")
    }

    /// Local model vault path (`data/models/`).
    pub fn models_dir(&self) -> PathBuf {
        self.data_dir.join("models")
    }

    pub fn model_catalog_meta(&self) -> &BuiltinCatalogMeta {
        &self.model_catalog_meta
    }

    /// Local model manager (vault registry, downloads, inference).
    pub fn model_manager(&self) -> &Arc<AsyncMutex<LocalModelManager>> {
        &self.model_manager
    }

    pub fn model_provider(&self) -> &SharedModelProvider {
        &self.model_provider
    }

    /// Embedded llama.cpp runtime supervisor.
    pub fn runtime_supervisor(&self) -> &Arc<AsyncMutex<RuntimeSupervisor>> {
        &self.runtime_supervisor
    }

    /// Default llama-server base URL for legacy install IPC fields.
    pub async fn ollama_base_url(&self) -> String {
        self.runtime_supervisor.lock().await.base_url().to_string()
    }

    /// Auth engine configuration (bundled Playwright paths resolved at startup).
    pub fn auth_engine_config(&self) -> &AuthEngineConfig {
        &self.auth_engine_config
    }
}
