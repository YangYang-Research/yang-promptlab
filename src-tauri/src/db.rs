//! Database bootstrap for the desktop backend integration layer.
//!
//! Resolves the SQLite location and opens the database (running migrations)
//! during application startup. Kept separate from `lib.rs` so it can be unit and
//! integration tested without a Tauri runtime.

use std::path::{Path, PathBuf};

use aisec_core::{AisecError, AisecResult};
use aisec_storage::Database;
use tracing::info;

/// Environment variable that overrides the database file location.
pub const DB_PATH_ENV: &str = "AISEC_DB_PATH";

/// Default database file name inside the application data directory.
pub const DB_FILENAME: &str = "aisec.db";

/// Resolve the database path: `AISEC_DB_PATH` if set (and non-empty), otherwise
/// `<data_dir>/aisec.db`.
pub fn resolve_db_path(data_dir: &Path) -> PathBuf {
    match std::env::var(DB_PATH_ENV) {
        Ok(custom) if !custom.trim().is_empty() => PathBuf::from(custom.trim()),
        _ => data_dir.join(DB_FILENAME),
    }
}

/// Open the SQLite database at `path`, creating parent directories as needed and
/// applying migrations. This is the single startup entry point used by the Tauri
/// `setup` hook.
pub async fn open_database(path: &Path) -> AisecResult<Database> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(AisecError::from)?;
        }
    }

    info!(path = %path.display(), "opening SQLite database");
    let db = Database::connect_path(path).await?;
    info!(path = %path.display(), "database ready (migrations applied)");
    Ok(db)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_db_path_defaults_to_data_dir() {
        // Ensure the env override is not set for this assertion.
        std::env::remove_var(DB_PATH_ENV);
        let p = resolve_db_path(Path::new("/var/lib/aisec"));
        assert_eq!(p, PathBuf::from("/var/lib/aisec/aisec.db"));
    }

    #[test]
    fn resolve_db_path_honors_env_override() {
        std::env::set_var(DB_PATH_ENV, "/custom/place.db");
        let p = resolve_db_path(Path::new("/ignored"));
        assert_eq!(p, PathBuf::from("/custom/place.db"));
        std::env::remove_var(DB_PATH_ENV);
    }
}
