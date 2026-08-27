use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::ConnectOptions;

use promptlab_core::{PromptLabError, PromptLabResult};

use crate::error::StorageResultExt;
use crate::repositories::Repositories;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// SQLite connection pool with applied migrations.
#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Open (or create) a database at `database_url` and run pending migrations.
    ///
    /// Use `sqlite://path/to/promptlab.db` for file-backed storage or
    /// `sqlite::memory:` for ephemeral databases (tests).
    pub async fn connect(database_url: &str) -> PromptLabResult<Self> {
        let mut options: SqliteConnectOptions = database_url.parse().map_storage()?;
        options = options
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

        let options = options.disable_statement_logging();

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_storage()?;

        MIGRATOR
            .run(&pool)
            .await
            .map_err(|err| {
                promptlab_core::PromptLabError::storage(format!(
                    "database migration failed (schema incompatible or corrupt): {err}"
                ))
            })?;

        tracing::debug!(%database_url, "database connected and migrations applied");

        Ok(Self { pool })
    }

    /// Open (or create) a file-backed database at `path` and run migrations.
    ///
    /// Unlike [`Database::connect`], this takes a filesystem path directly and is
    /// robust to paths containing spaces or characters that are awkward to encode
    /// in a `sqlite://` URL. The parent directory must already exist.
    ///
    /// Uses WAL journaling for concurrency, but **falls back to a rollback
    /// journal** (`TRUNCATE`) when the filesystem does not support WAL's
    /// shared-memory files — some overlay/network filesystems return a disk I/O
    /// error otherwise. This keeps startup reliable across environments.
    pub async fn connect_path(path: impl AsRef<Path>) -> PromptLabResult<Self> {
        let path = path.as_ref();
        match Self::open_file(path, sqlx::sqlite::SqliteJournalMode::Wal).await {
            Ok(db) => {
                tracing::debug!(path = %path.display(), "database connected (WAL) and migrations applied");
                Ok(db)
            }
            Err(wal_err) => {
                tracing::warn!(
                    error = %wal_err.client_message(),
                    "WAL journal unavailable on this filesystem; retrying with TRUNCATE"
                );
                // Remove partial WAL side files before retrying so recovery is clean.
                for suffix in ["-wal", "-shm"] {
                    let mut side = std::ffi::OsString::from(path.as_os_str());
                    side.push(suffix);
                    let _ = std::fs::remove_file(std::path::PathBuf::from(side));
                }
                let db = Self::open_file(path, sqlx::sqlite::SqliteJournalMode::Truncate).await?;
                tracing::debug!(path = %path.display(), "database connected (TRUNCATE) and migrations applied");
                Ok(db)
            }
        }
    }

    async fn open_file(path: &Path, journal_mode: sqlx::sqlite::SqliteJournalMode) -> PromptLabResult<Self> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(journal_mode)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .disable_statement_logging();

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_storage()?;

        MIGRATOR
            .run(&pool)
            .await
            .map_err(|err| {
                PromptLabError::storage(format!(
                    "database migration failed (schema incompatible or corrupt): {err}"
                ))
            })?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn repositories(&self) -> Repositories {
        Repositories::new(self.pool.clone())
    }

    /// Returns true once the connection pool has been closed.
    pub fn is_closed(&self) -> bool {
        self.pool.is_closed()
    }

    /// Gracefully close the connection pool, flushing in-flight work.
    ///
    /// Idempotent: closing an already-closed pool is a no-op.
    pub async fn close(&self) {
        if !self.pool.is_closed() {
            self.pool.close().await;
        }
    }
}

#[cfg(test)]
pub mod test_utils {
    use super::*;

    pub async fn test_database() -> Database {
        Database::connect("sqlite::memory:")
            .await
            .expect("in-memory database should initialize")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CreateProject;
    use crate::repositories::ProjectRepository;

    #[tokio::test]
    async fn connect_path_creates_file_and_runs_migrations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("promptlab.db");
        let db = Database::connect_path(&path).await.expect("connect_path");
        assert!(path.exists(), "database file should be created");

        // Migrations applied -> repositories are usable.
        let project = db
            .repositories()
            .projects()
            .create(CreateProject {
                name: "p".into(),
                description: None,
            })
            .await
            .expect("create project");
        assert_eq!(db.repositories().projects().list().await.unwrap().len(), 1);
        assert_eq!(project.name, "p");

        assert!(!db.is_closed());
        db.close().await;
        assert!(db.is_closed());
        db.close().await; // idempotent
    }
}
