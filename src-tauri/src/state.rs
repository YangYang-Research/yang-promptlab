use std::path::{Path, PathBuf};

use aisec_core::LogGuard;
use aisec_storage::{Database, Repositories};

/// Shared application state managed by Tauri and accessible from commands.
///
/// Holds the open [`Database`] (SQLite connection pool with migrations applied),
/// the application data directory (used to resolve report output), and keeps the
/// logging guard alive for the lifetime of the process.
pub struct AppState {
    db: Database,
    data_dir: PathBuf,
    _log_guard: LogGuard,
}

impl AppState {
    pub fn new(db: Database, data_dir: PathBuf, log_guard: LogGuard) -> Self {
        Self {
            db,
            data_dir,
            _log_guard: log_guard,
        }
    }

    /// The shared database handle (cheap clone of the connection pool).
    pub fn database(&self) -> &Database {
        &self.db
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
}
