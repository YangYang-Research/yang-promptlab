use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::ConnectOptions;

use aisec_core::AisecResult;

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
    /// Use `sqlite://path/to/aisec.db` for file-backed storage or
    /// `sqlite::memory:` for ephemeral databases (tests).
    pub async fn connect(database_url: &str) -> AisecResult<Self> {
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
            .map_err(|err| aisec_core::AisecError::internal(format!("migration failed: {err}")))?;

        tracing::debug!(%database_url, "database connected and migrations applied");

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn repositories(&self) -> Repositories {
        Repositories::new(self.pool.clone())
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
