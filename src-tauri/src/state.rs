use std::path::{Path, PathBuf};

use aisec_auth::AuthEngineConfig;
use aisec_core::LogGuard;
use aisec_models::LocalModelManager;
use aisec_storage::{Database, Repositories};
use tauri::async_runtime::Mutex as AsyncMutex;

use crate::jobs::ScanJobManager;

/// Shared application state managed by Tauri and accessible from commands.
pub struct AppState {
    db: Database,
    data_dir: PathBuf,
    jobs: ScanJobManager,
    auth_engine_config: AuthEngineConfig,
    model_manager: AsyncMutex<LocalModelManager>,
    _log_guard: LogGuard,
}

impl AppState {
    pub fn new(
        db: Database,
        data_dir: PathBuf,
        log_guard: LogGuard,
        auth_engine_config: AuthEngineConfig,
    ) -> Self {
        let model_manager = crate::judge_config::open_model_manager(&data_dir)
            .expect("failed to initialize local model manager");

        Self {
            db,
            data_dir,
            jobs: ScanJobManager::default(),
            auth_engine_config,
            model_manager: AsyncMutex::new(model_manager),
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
    ///
    /// `Repositories` is the repository manager: it exposes per-entity
    /// repositories (`projects()`, `targets()`, `scans()`, `findings()`,
    /// `reports()`, …) that share the single pooled connection.
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

    /// Local model manager (vault registry, downloads, inference).
    pub fn model_manager(&self) -> &AsyncMutex<LocalModelManager> {
        &self.model_manager
    }

    /// Auth engine configuration (bundled Playwright paths resolved at startup).
    pub fn auth_engine_config(&self) -> &AuthEngineConfig {
        &self.auth_engine_config
    }
}
