use sqlx::SqlitePool;

mod attack_catalog;
mod attack_result;
mod auth;
mod endpoint;
mod finding;
mod model;
mod payload;
mod plugin;
mod project;
mod report;
mod scan;
mod target;

pub use attack_catalog::SqliteAttackCatalogRepository;
pub use attack_result::SqliteAttackResultRepository;
pub use auth::{
    SqliteAuthProfileRepository, SqliteAuthRecordingRepository, SqliteAuthSessionRepository,
};
pub use endpoint::SqliteEndpointRepository;
pub use finding::SqliteFindingRepository;
pub use model::SqliteModelRepository;
pub use payload::SqlitePayloadRepository;
pub use plugin::SqlitePluginRepository;
pub use project::SqliteProjectRepository;
pub use report::SqliteReportRepository;
pub use scan::SqliteScanRepository;
pub use target::SqliteTargetRepository;

/// Factory for SQLite-backed repositories sharing a connection pool.
#[derive(Clone)]
pub struct Repositories {
    pool: SqlitePool,
}

impl Repositories {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn projects(&self) -> SqliteProjectRepository {
        SqliteProjectRepository::new(self.pool.clone())
    }

    pub fn targets(&self) -> SqliteTargetRepository {
        SqliteTargetRepository::new(self.pool.clone())
    }

    pub fn scans(&self) -> SqliteScanRepository {
        SqliteScanRepository::new(self.pool.clone())
    }

    pub fn findings(&self) -> SqliteFindingRepository {
        SqliteFindingRepository::new(self.pool.clone())
    }

    pub fn endpoints(&self) -> SqliteEndpointRepository {
        SqliteEndpointRepository::new(self.pool.clone())
    }

    pub fn payloads(&self) -> SqlitePayloadRepository {
        SqlitePayloadRepository::new(self.pool.clone())
    }

    pub fn attack_results(&self) -> SqliteAttackResultRepository {
        SqliteAttackResultRepository::new(self.pool.clone())
    }

    pub fn reports(&self) -> SqliteReportRepository {
        SqliteReportRepository::new(self.pool.clone())
    }

    pub fn models(&self) -> SqliteModelRepository {
        SqliteModelRepository::new(self.pool.clone())
    }

    pub fn plugins(&self) -> SqlitePluginRepository {
        SqlitePluginRepository::new(self.pool.clone())
    }

    pub fn attack_catalog(&self) -> SqliteAttackCatalogRepository {
        SqliteAttackCatalogRepository::new(self.pool.clone())
    }

    pub fn auth_profiles(&self) -> SqliteAuthProfileRepository {
        SqliteAuthProfileRepository::new(self.pool.clone())
    }

    pub fn auth_sessions(&self) -> SqliteAuthSessionRepository {
        SqliteAuthSessionRepository::new(self.pool.clone())
    }

    pub fn auth_recordings(&self) -> SqliteAuthRecordingRepository {
        SqliteAuthRecordingRepository::new(self.pool.clone())
    }
}
