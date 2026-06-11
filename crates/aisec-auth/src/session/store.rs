use std::path::{Path, PathBuf};

use aisec_core::{AisecError, AisecResult};
use aisec_storage::{
    AuthProfileRepository, AuthRecordingRepository, AuthSessionRepository, CreateAuthProfile,
    CreateAuthRecordingRecord, CreateAuthSessionRecord, Database, UpdateAuthSessionRecord,
};
use tracing::debug;

use crate::types::{
    AuthMethod, AuthProfile, AuthSession, CookieRecord, ExtractedToken, LoginRecording,
    PlaywrightStorageState, RecordedStep, SessionStatus,
};

/// Persists auth sessions, recordings, and Playwright storageState files.
pub struct SessionStore {
    db: Database,
    vault_dir: PathBuf,
}

impl SessionStore {
    pub async fn new(db: Database, vault_dir: impl Into<PathBuf>) -> AisecResult<Self> {
        let vault_dir = vault_dir.into();
        tokio::fs::create_dir_all(&vault_dir)
            .await
            .map_err(AisecError::from)?;
        Ok(Self { db, vault_dir })
    }

    pub fn vault_dir(&self) -> &Path {
        &self.vault_dir
    }

    pub async fn save_storage_state(
        &self,
        session_id: &str,
        state: &PlaywrightStorageState,
    ) -> AisecResult<PathBuf> {
        let path = self
            .vault_dir
            .join(format!("{session_id}.storage.json"));
        let json = serde_json::to_string_pretty(state)
            .map_err(|e| AisecError::internal(e.to_string()))?;
        tokio::fs::write(&path, json)
            .await
            .map_err(AisecError::from)?;
        debug!(%session_id, path = %path.display(), "saved storage state");
        Ok(path)
    }

    pub async fn load_storage_state(&self, path: &Path) -> AisecResult<PlaywrightStorageState> {
        let data = tokio::fs::read_to_string(path)
            .await
            .map_err(AisecError::from)?;
        serde_json::from_str(&data).map_err(|e| AisecError::internal(e.to_string()))
    }

    pub async fn persist_session(
        &self,
        profile_id: &str,
        cookies: &[CookieRecord],
        tokens: &[ExtractedToken],
        storage_state_path: Option<PathBuf>,
    ) -> AisecResult<AuthSession> {
        let record = self
            .db
            .repositories()
            .auth_sessions()
            .create(CreateAuthSessionRecord {
                profile_id: profile_id.to_string(),
                status: Some(SessionStatus::Active.as_str().to_string()),
                cookies_json: Some(serde_json::to_value(cookies).unwrap()),
                tokens_json: Some(serde_json::to_value(tokens).unwrap()),
                storage_state_path: storage_state_path.map(|p| p.to_string_lossy().into_owned()),
                expires_at: None,
            })
            .await?;

        Ok(AuthSession {
            id: record.id,
            profile_id: profile_id.to_string(),
            status: SessionStatus::Active,
            cookies: cookies.to_vec(),
            tokens: tokens.to_vec(),
            storage_state_path: record.storage_state_path,
            expires_at: record.expires_at,
        })
    }

    pub async fn update_session_cookies(
        &self,
        session_id: &str,
        cookies: &[CookieRecord],
    ) -> AisecResult<()> {
        self.db
            .repositories()
            .auth_sessions()
            .update(
                session_id,
                UpdateAuthSessionRecord {
                    cookies_json: Some(serde_json::to_value(cookies).unwrap()),
                    ..Default::default()
                },
            )
            .await?;
        Ok(())
    }

    pub async fn update_session_tokens(
        &self,
        session_id: &str,
        tokens: &[ExtractedToken],
    ) -> AisecResult<()> {
        self.db
            .repositories()
            .auth_sessions()
            .update(
                session_id,
                UpdateAuthSessionRecord {
                    tokens_json: Some(serde_json::to_value(tokens).unwrap()),
                    ..Default::default()
                },
            )
            .await?;
        Ok(())
    }

    pub async fn get_session(&self, session_id: &str) -> AisecResult<AuthSession> {
        let record = self
            .db
            .repositories()
            .auth_sessions()
            .get(session_id)
            .await?;

        let cookies: Vec<CookieRecord> = record
            .cookies_json
            .as_deref()
            .map(|j| serde_json::from_str(j))
            .transpose()
            .map_err(|e| AisecError::internal(e.to_string()))?
            .unwrap_or_default();

        let tokens: Vec<ExtractedToken> = record
            .tokens_json
            .as_deref()
            .map(|j| serde_json::from_str(j))
            .transpose()
            .map_err(|e| AisecError::internal(e.to_string()))?
            .unwrap_or_default();

        let status = match record.status.as_str() {
            "expired" => SessionStatus::Expired,
            "revoked" => SessionStatus::Revoked,
            _ => SessionStatus::Active,
        };

        Ok(AuthSession {
            id: record.id,
            profile_id: record.profile_id,
            status,
            cookies,
            tokens,
            storage_state_path: record.storage_state_path,
            expires_at: record.expires_at,
        })
    }

    pub async fn save_recording(
        &self,
        profile_id: &str,
        steps: &[RecordedStep],
        storage_state_path: Option<PathBuf>,
        metadata: serde_json::Value,
    ) -> AisecResult<LoginRecording> {
        let record = self
            .db
            .repositories()
            .auth_recordings()
            .create(CreateAuthRecordingRecord {
                profile_id: profile_id.to_string(),
                steps_json: serde_json::to_value(steps).unwrap(),
                storage_state_path: storage_state_path.map(|p| p.to_string_lossy().into_owned()),
                metadata_json: Some(metadata),
            })
            .await?;

        Ok(LoginRecording {
            id: record.id,
            profile_id: record.profile_id,
            steps: serde_json::from_str(&record.steps_json).unwrap_or_default(),
            storage_state_path: record.storage_state_path,
            metadata: record
                .metadata_json
                .and_then(|m| serde_json::from_str(&m).ok())
                .unwrap_or(serde_json::json!({})),
        })
    }

    pub async fn update_storage_path(
        &self,
        session_id: &str,
        path: impl AsRef<Path>,
    ) -> AisecResult<()> {
        self.db
            .repositories()
            .auth_sessions()
            .update(
                session_id,
                UpdateAuthSessionRecord {
                    storage_state_path: Some(path.as_ref().to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await?;
        Ok(())
    }

    pub async fn create_profile(&self, profile: &AuthProfile) -> AisecResult<AuthProfile> {
        let stored = self
            .db
            .repositories()
            .auth_profiles()
            .create(CreateAuthProfile {
                project_id: profile.project_id.clone(),
                name: profile.name.clone(),
                method: profile.method.as_str().to_string(),
                config_json: serde_json::to_value(&profile.config).unwrap(),
            })
            .await?;

        Ok(AuthProfile {
            id: stored.id,
            project_id: stored.project_id,
            name: stored.name,
            method: AuthMethod::parse(&stored.method).unwrap_or(AuthMethod::UsernamePassword),
            config: serde_json::from_str(&stored.config_json).unwrap(),
        })
    }
}
