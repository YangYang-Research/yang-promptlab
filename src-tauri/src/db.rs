//! Database bootstrap for the PromptLab desktop backend.

use std::path::Path;

use promptlab_core::PromptLabResult;
use promptlab_storage::Database;
use tracing::info;

pub use promptlab_core::{resolve_db_path, DB_FILENAME, DB_PATH_ENV};

/// Open the SQLite database at `path`, creating parent directories as needed and
/// applying migrations.
pub async fn open_database(path: &Path) -> PromptLabResult<Database> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(promptlab_core::PromptLabError::from)?;
        }
    }

    info!(path = %path.display(), "opening SQLite database");
    let db = Database::connect_path(path).await?;
    info!(path = %path.display(), "database ready (schema applied)");
    Ok(db)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn resolve_db_path_default_and_override() {
        std::env::remove_var(DB_PATH_ENV);
        assert_eq!(
            resolve_db_path(Path::new("/root/.promptlab/workspaces")),
            PathBuf::from("/root/.promptlab/workspaces/promptlab.db")
        );

        std::env::set_var(DB_PATH_ENV, "/custom/place.db");
        assert_eq!(
            resolve_db_path(Path::new("/ignored")),
            PathBuf::from("/custom/place.db")
        );

        std::env::remove_var(DB_PATH_ENV);
    }
}
