use aisec_core::LogGuard;
use aisec_storage::{Database, Repositories};

/// Shared application state managed by Tauri and accessible from commands.
///
/// Holds the open [`Database`] (SQLite connection pool with migrations applied)
/// and keeps the logging guard alive for the lifetime of the process.
pub struct AppState {
    db: Database,
    _log_guard: LogGuard,
}

impl AppState {
    pub fn new(db: Database, log_guard: LogGuard) -> Self {
        Self {
            db,
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
    /// repositories (`projects()`, `targets()`, `scans()`, `findings()`, …) that
    /// share the single pooled connection.
    pub fn repositories(&self) -> Repositories {
        self.db.repositories()
    }
}
