use std::path::{Path, PathBuf};

use aisec_auth::{
    AuthEngine, AuthEngineConfig, AuthSession, RecordLoginOptions, SessionValidationStatus,
};
use aisec_storage::Database;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::error::{BrowserError, BrowserResult};
use crate::paths::auth_sessions_dir;

/// Metadata persisted alongside Playwright storage state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserSessionRecord {
    pub session_id: String,
    pub name: String,
    pub recorded_at: OffsetDateTime,
    pub expires_at: Option<OffsetDateTime>,
    pub status: BrowserSessionStatus,
    pub storage_state_path: PathBuf,
    pub metadata_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserSessionStatus {
    Valid,
    ExpiringSoon,
    Expired,
}

impl BrowserSessionStatus {
    pub fn from_validation(status: SessionValidationStatus) -> Self {
        match status {
            SessionValidationStatus::Valid => Self::Valid,
            SessionValidationStatus::ExpiringSoon => Self::ExpiringSoon,
            SessionValidationStatus::Expired => Self::Expired,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::ExpiringSoon => "expiring_soon",
            Self::Expired => "expired",
        }
    }
}

/// Browser session lifecycle manager backed by `aisec-auth` storage.
pub struct BrowserSessionManager {
    sessions_dir: PathBuf,
    engine: AuthEngine,
}

impl BrowserSessionManager {
    pub async fn new(
        db: Database,
        data_dir: impl AsRef<Path>,
        config: AuthEngineConfig,
    ) -> BrowserResult<Self> {
        let sessions_dir = auth_sessions_dir(data_dir.as_ref());
        tokio::fs::create_dir_all(&sessions_dir)
            .await
            .map_err(|err| BrowserError::Storage(err.to_string()))?;

        let store = aisec_auth::SessionStore::new(db, &sessions_dir).await?;
        let engine = AuthEngine::new(
            config.with_vault_dir(sessions_dir.clone()),
            store,
            None,
        )
        .await?;

        Ok(Self {
            sessions_dir,
            engine,
        })
    }

    pub fn sessions_dir(&self) -> &Path {
        &self.sessions_dir
    }

    pub async fn record_session(
        &self,
        login_url: &str,
        options: RecordLoginOptions,
    ) -> BrowserResult<()> {
        self.engine
            .begin_interactive_recording(login_url, options)
            .await?;
        Ok(())
    }

    pub async fn finish_record_session(
        &self,
        profile_id: &str,
        name: &str,
    ) -> BrowserResult<BrowserSessionRecord> {
        let (session, _recording) = self
            .engine
            .finish_interactive_recording(profile_id)
            .await?;

        let metadata_path = self.sessions_dir.join(format!("{}.meta.json", session.id));
        let storage_state_path = session
            .storage_state_path
            .as_ref()
            .map(PathBuf::from)
            .ok_or_else(|| BrowserError::Storage("missing storage state path".into()))?;

        let record = BrowserSessionRecord {
            session_id: session.id.clone(),
            name: name.to_string(),
            recorded_at: session.created_at,
            expires_at: session.expires_at,
            status: BrowserSessionStatus::from_validation(session.validation_status),
            storage_state_path,
            metadata_path: metadata_path.clone(),
        };

        let json = serde_json::to_string_pretty(&record)
            .map_err(|err| BrowserError::Storage(err.to_string()))?;
        tokio::fs::write(&metadata_path, json)
            .await
            .map_err(|err| BrowserError::Storage(err.to_string()))?;

        Ok(record)
    }

    pub async fn load_session(&self, session_id: &str) -> BrowserResult<BrowserSessionRecord> {
        let metadata_path = self.sessions_dir.join(format!("{session_id}.meta.json"));
        if metadata_path.exists() {
            let data = tokio::fs::read_to_string(&metadata_path)
                .await
                .map_err(|err| BrowserError::Storage(err.to_string()))?;
            return serde_json::from_str(&data).map_err(|err| BrowserError::Storage(err.to_string()));
        }

        let session = self.engine.store().get_session(session_id).await?;
        Ok(BrowserSessionRecord {
            session_id: session.id,
            name: session.profile_id,
            recorded_at: session.created_at,
            expires_at: session.expires_at,
            status: BrowserSessionStatus::from_validation(session.validation_status),
            storage_state_path: session
                .storage_state_path
                .map(PathBuf::from)
                .ok_or_else(|| BrowserError::Storage("missing storage state".into()))?,
            metadata_path,
        })
    }

    pub async fn validate_session(
        &self,
        session_id: &str,
        _probe_url: Option<&str>,
    ) -> BrowserResult<AuthSession> {
        let session = self.engine.store().get_session(session_id).await?;
        if session.validation_status == SessionValidationStatus::Expired {
            return Err(BrowserError::Expired);
        }
        Ok(session)
    }

    pub async fn delete_session(&self, session_id: &str) -> BrowserResult<()> {
        if let Ok(record) = self.load_session(session_id).await {
            let _ = tokio::fs::remove_file(record.metadata_path).await;
            let _ = tokio::fs::remove_file(record.storage_state_path).await;
        }
        self.engine.store().delete_session(session_id).await?;
        Ok(())
    }
}
