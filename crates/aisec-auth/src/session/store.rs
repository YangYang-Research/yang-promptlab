use std::path::{Path, PathBuf};

use aisec_core::{AisecError, AisecResult};
use aisec_storage::{
    AuthProfileRepository, AuthRecordingRepository, AuthSessionRepository, CreateAuthProfile,
    CreateAuthRecordingRecord, CreateAuthSessionRecord, Database, UpdateAuthSessionRecord,
};
use tracing::debug;

use crate::secrets::{
    session_secrets_from_json, session_secrets_to_json, store_auth_config_secrets,
    CredentialReferenceId, EncryptedVault, SecretScope, SecretStore,
};
use crate::types::{
    AuthMethod, AuthProfile, AuthSession, CookieRecord, ExtractedToken, LoginRecording,
    PlaywrightStorageState, RecordedStep, SessionStatus, SessionValidationStatus,
};

/// Persists auth sessions, recordings, and encrypted Playwright storageState artifacts.
pub struct SessionStore {
    db: Database,
    vault_dir: PathBuf,
    secrets: SecretStore,
    encrypted_vault: EncryptedVault,
}

impl SessionStore {
    pub async fn new(db: Database, vault_dir: impl Into<PathBuf>) -> AisecResult<Self> {
        let vault_dir = vault_dir.into();
        tokio::fs::create_dir_all(&vault_dir)
            .await
            .map_err(AisecError::from)?;
        let secrets = SecretStore::new()?;
        let encrypted_vault = EncryptedVault::new(&secrets, vault_dir.clone())?;
        Ok(Self {
            db,
            vault_dir,
            secrets,
            encrypted_vault,
        })
    }

    pub fn vault_dir(&self) -> &Path {
        &self.vault_dir
    }

    pub fn secrets(&self) -> &SecretStore {
        &self.secrets
    }

    pub fn encrypted_vault(&self) -> &EncryptedVault {
        &self.encrypted_vault
    }

    pub async fn save_storage_state(
        &self,
        session_id: &str,
        state: &PlaywrightStorageState,
    ) -> AisecResult<PathBuf> {
        let json = serde_json::to_string_pretty(state)
            .map_err(|e| AisecError::internal(e.to_string()))?;
        let path = self
            .encrypted_vault
            .write_json(session_id, &json)
            .await?;
        debug!(%session_id, path = %path.display(), "saved encrypted storage state");
        Ok(path)
    }

    pub async fn load_storage_state(&self, path: &Path) -> AisecResult<PlaywrightStorageState> {
        let data = if path.extension().and_then(|e| e.to_str()) == Some("enc") {
            self.encrypted_vault.read_json(path).await?
        } else {
            tokio::fs::read_to_string(path)
                .await
                .map_err(AisecError::from)?
        };
        serde_json::from_str(&data).map_err(|e| AisecError::internal(e.to_string()))
    }

    pub async fn persist_session(
        &self,
        profile_id: &str,
        cookies: &[CookieRecord],
        tokens: &[ExtractedToken],
        storage_state_path: Option<PathBuf>,
    ) -> AisecResult<AuthSession> {
        let secrets_json = session_secrets_to_json(cookies, tokens)?;
        let credential_reference_id = self
            .secrets
            .store(SecretScope::Session, &secrets_json)?
            .to_string();

        let record = self
            .db
            .repositories()
            .auth_sessions()
            .create(CreateAuthSessionRecord {
                profile_id: profile_id.to_string(),
                status: Some(SessionStatus::Active.as_str().to_string()),
                cookies_json: None,
                tokens_json: None,
                credential_reference_id: Some(credential_reference_id.clone()),
                storage_state_path: storage_state_path.map(|p| p.to_string_lossy().into_owned()),
                expires_at: crate::session::manager::earliest_cookie_expiry(cookies),
                validation_status: Some(SessionValidationStatus::Valid.as_str().to_string()),
                user_identity: crate::session::manager::infer_user_identity(tokens, cookies),
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
            validation_status: SessionValidationStatus::parse(&record.validation_status),
            last_validated_at: record.last_validated_at,
            user_identity: record.user_identity,
            created_at: record.created_at,
        })
    }

    async fn persist_session_secrets(
        &self,
        _session_id: &str,
        credential_reference_id: Option<&str>,
        cookies: &[CookieRecord],
        tokens: &[ExtractedToken],
    ) -> AisecResult<String> {
        let secrets_json = session_secrets_to_json(cookies, tokens)?;
        if let Some(existing) = credential_reference_id {
            let id = CredentialReferenceId::parse(existing);
            self.secrets
                .store_with_id(SecretScope::Session, &id, &secrets_json)?;
            Ok(existing.to_string())
        } else {
            Ok(self.secrets.store(SecretScope::Session, &secrets_json)?.to_string())
        }
    }

    pub async fn update_session_cookies(
        &self,
        session_id: &str,
        cookies: &[CookieRecord],
    ) -> AisecResult<()> {
        let session = self.get_session(session_id).await?;
        let record = self.db.repositories().auth_sessions().get(session_id).await?;
        let cred_id = self
            .persist_session_secrets(
                session_id,
                record.credential_reference_id.as_deref(),
                cookies,
                &session.tokens,
            )
            .await?;
        self.db
            .repositories()
            .auth_sessions()
            .update(
                session_id,
                UpdateAuthSessionRecord {
                    credential_reference_id: Some(cred_id),
                    cookies_json: None,
                    tokens_json: None,
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
        let session = self.get_session(session_id).await?;
        let record = self.db.repositories().auth_sessions().get(session_id).await?;
        let cred_id = self
            .persist_session_secrets(
                session_id,
                record.credential_reference_id.as_deref(),
                &session.cookies,
                tokens,
            )
            .await?;
        self.db
            .repositories()
            .auth_sessions()
            .update(
                session_id,
                UpdateAuthSessionRecord {
                    credential_reference_id: Some(cred_id),
                    cookies_json: None,
                    tokens_json: None,
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

        let (cookies, tokens) = if let Some(cred_id) = &record.credential_reference_id {
            session_secrets_from_json(
                &self
                    .secrets
                    .load(SecretScope::Session, &CredentialReferenceId::parse(cred_id))?,
            )?
        } else {
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
            (cookies, tokens)
        };

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
            validation_status: SessionValidationStatus::parse(&record.validation_status),
            last_validated_at: record.last_validated_at,
            user_identity: record.user_identity,
            created_at: record.created_at,
        })
    }

    pub async fn update_validation(
        &self,
        session_id: &str,
        validation_status: SessionValidationStatus,
        last_validated_at: Option<time::OffsetDateTime>,
        user_identity: Option<&str>,
        expires_at: Option<time::OffsetDateTime>,
    ) -> AisecResult<()> {
        self.db
            .repositories()
            .auth_sessions()
            .update(
                session_id,
                UpdateAuthSessionRecord {
                    validation_status: Some(validation_status.as_str().to_string()),
                    last_validated_at,
                    user_identity: user_identity.map(str::to_string),
                    expires_at,
                    ..Default::default()
                },
            )
            .await?;
        Ok(())
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
        let mut config = profile.config.clone();
        store_auth_config_secrets(&mut config, &self.secrets)?;

        let stored = self
            .db
            .repositories()
            .auth_profiles()
            .create(CreateAuthProfile {
                project_id: profile.project_id.clone(),
                name: profile.name.clone(),
                method: profile.method.as_str().to_string(),
                config_json: serde_json::to_value(&config).unwrap(),
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

    pub async fn delete_session(&self, session_id: &str) -> AisecResult<()> {
        if let Ok(session) = self.get_session(session_id).await {
            if let Some(path) = session.storage_state_path.as_deref().map(Path::new) {
                let _ = self.encrypted_vault.delete_artifact(path).await;
            }
        }
        if let Ok(record) = self.db.repositories().auth_sessions().get(session_id).await {
            if let Some(cred_id) = record.credential_reference_id {
                let _ = self
                    .secrets
                    .delete(SecretScope::Session, &CredentialReferenceId::parse(cred_id));
            }
        }
        self.db
            .repositories()
            .auth_sessions()
            .delete(session_id)
            .await
    }
}
