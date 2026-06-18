use aisec_core::{AisecError, AisecResult};
use aisec_storage::{
    AuthProfileRepository, AuthSessionRepository, Database, TargetRepository,
    UpdateAuthSessionRecord,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::types::{AuthConfig, CookieRecord, ExtractedToken, PlaywrightStorageState};

use super::descriptor::sanitize_target_descriptor;
use super::store::{CredentialReferenceId, SecretScope, SecretStore};
use super::vault::EncryptedVault;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionSecrets {
    cookies: Vec<CookieRecord>,
    tokens: Vec<ExtractedToken>,
}

/// Migrate legacy plaintext session/profile secrets into the OS keychain.
pub async fn migrate_legacy_auth_data(db: &Database, secrets: &SecretStore) -> AisecResult<u32> {
    let repos = db.repositories();
    let mut migrated = 0u32;

    let sessions = repos
        .auth_sessions()
        .list_legacy_with_plaintext_secrets()
        .await?;

    for session in sessions {
        let cookies: Vec<CookieRecord> = session
            .cookies_json
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();
        let tokens: Vec<ExtractedToken> = session
            .tokens_json
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();

        if cookies.is_empty() && tokens.is_empty() && session.credential_reference_id.is_some() {
            continue;
        }

        let payload = SessionSecrets { cookies, tokens };
        let json = serde_json::to_string(&payload)
            .map_err(|err| AisecError::internal(err.to_string()))?;
        let cred_id = if let Some(existing) = session.credential_reference_id.clone() {
            let id = CredentialReferenceId::parse(existing);
            secrets.store_with_id(SecretScope::Session, &id, &json)?;
            id
        } else {
            secrets.store(SecretScope::Session, &json)?
        };

        repos
            .auth_sessions()
            .apply_secure_migration(&session.id, cred_id.as_str())
            .await?;
        migrated += 1;
    }

    let profiles = repos.auth_profiles().list_all().await?;
    for profile in profiles {
        let mut config: serde_json::Value =
            serde_json::from_str(&profile.config_json).unwrap_or(serde_json::json!({}));
        if migrate_profile_config(&mut config, secrets)? {
            repos
                .auth_profiles()
                .update_config_and_reference(
                    &profile.id,
                    &config,
                    profile.credential_reference_id.as_deref(),
                )
                .await?;
            migrated += 1;
        }
    }

    if migrated > 0 {
        info!(count = migrated, "migrated legacy auth secrets to secure storage");
    }
    Ok(migrated)
}

pub async fn migrate_legacy_target_descriptors(db: &Database, secrets: &SecretStore) -> AisecResult<u32> {
    let repos = db.repositories();
    let targets = repos.targets().list_all().await?;
    let mut migrated = 0u32;

    for target in targets {
        match sanitize_target_descriptor(&target.descriptor_json, secrets) {
            Ok((sanitized, changed)) if changed => {
                repos
                    .targets()
                    .update_descriptor(&target.id, &sanitized)
                    .await?;
                migrated += 1;
            }
            Ok(_) => {}
            Err(err) => {
                warn!(target_id = %target.id, error = %err, "target descriptor migration skipped");
            }
        }
    }

    if migrated > 0 {
        info!(count = migrated, "migrated target descriptor secrets to secure storage");
    }
    Ok(migrated)
}

/// Re-encrypt legacy plaintext Playwright storageState files and update session paths.
pub async fn migrate_legacy_storage_artifacts(
    db: &Database,
    data_dir: &Path,
    vault: &EncryptedVault,
) -> AisecResult<u32> {
    let legacy_dir = data_dir.join("auth-vault");
    let repos = db.repositories();
    let sessions = repos.auth_sessions().list_all().await?;
    let mut migrated = 0u32;

    for session in sessions {
        let Some(path_str) = session.storage_state_path else {
            continue;
        };
        let path = PathBuf::from(&path_str);
        if path.extension().and_then(|e| e.to_str()) == Some("enc") {
            continue;
        }

        let read_path = if path.is_file() {
            path
        } else {
            legacy_dir.join(format!("{}.storage.json", session.id))
        };
        if !read_path.is_file() {
            continue;
        }

        let plaintext = tokio::fs::read_to_string(&read_path)
            .await
            .map_err(AisecError::from)?;

        let _: PlaywrightStorageState = serde_json::from_str(&plaintext)
            .map_err(|err| AisecError::internal(format!("legacy storage state: {err}")))?;

        let encrypted_path = vault.write_json(&session.id, &plaintext).await?;
        repos
            .auth_sessions()
            .update(
                &session.id,
                UpdateAuthSessionRecord {
                    storage_state_path: Some(encrypted_path.to_string_lossy().into_owned()),
                    ..Default::default()
                },
            )
            .await?;
        migrated += 1;
    }

    if migrated > 0 {
        info!(count = migrated, "migrated legacy storage artifacts to encrypted vault");
    }
    Ok(migrated)
}

/// Result of migrating database-backed and on-disk session secrets.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretMigrationResult {
    pub auth_migrated: u32,
    pub targets_migrated: u32,
    pub storage_migrated: u32,
}

/// Run all SQLite + encrypted vault migrations (excluding judge config file).
pub async fn run_database_secret_migration(
    db: &Database,
    data_dir: &Path,
    secrets: &SecretStore,
    vault: &EncryptedVault,
) -> AisecResult<SecretMigrationResult> {
    let auth_migrated = migrate_legacy_auth_data(db, secrets).await?;
    let targets_migrated = migrate_legacy_target_descriptors(db, secrets).await?;
    let storage_migrated = migrate_legacy_storage_artifacts(db, data_dir, vault).await?;
    Ok(SecretMigrationResult {
        auth_migrated,
        targets_migrated,
        storage_migrated,
    })
}

/// Returns true when a profile config JSON still stores inline secrets.
pub fn profile_config_has_plaintext(config: &serde_json::Value) -> bool {
    let Some(obj) = config.as_object() else {
        return false;
    };
    for key in ["password", "token", "key"] {
        if obj
            .get(key)
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty())
        {
            return true;
        }
    }
    false
}

fn migrate_profile_config(
    config: &mut serde_json::Value,
    secrets: &SecretStore,
) -> AisecResult<bool> {
    let Some(obj) = config.as_object_mut() else {
        return Ok(false);
    };
    let mut changed = false;

    if let Some(password) = obj.remove("password") {
        if let Some(value) = password.as_str().filter(|v| !v.is_empty()) {
            let id = secrets.store(SecretScope::Profile, value)?;
            obj.insert(
                "password_credential_id".into(),
                serde_json::Value::String(id.to_string()),
            );
            changed = true;
        }
    }

    if let Some(token) = obj.remove("token") {
        if let Some(value) = token.as_str().filter(|v| !v.is_empty()) {
            let id = secrets.store(SecretScope::Profile, value)?;
            obj.insert(
                "token_credential_id".into(),
                serde_json::Value::String(id.to_string()),
            );
            changed = true;
        }
    }

    if let Some(key) = obj.remove("key") {
        if let Some(value) = key.as_str().filter(|v| !v.is_empty()) {
            let id = secrets.store(SecretScope::Profile, value)?;
            obj.insert(
                "key_credential_id".into(),
                serde_json::Value::String(id.to_string()),
            );
            changed = true;
        }
    }

    Ok(changed)
}

pub fn session_secrets_to_json(
    cookies: &[CookieRecord],
    tokens: &[ExtractedToken],
) -> AisecResult<String> {
    serde_json::to_string(&SessionSecrets {
        cookies: cookies.to_vec(),
        tokens: tokens.to_vec(),
    })
    .map_err(|err| AisecError::internal(err.to_string()))
}

pub fn session_secrets_from_json(json: &str) -> AisecResult<(Vec<CookieRecord>, Vec<ExtractedToken>)> {
    let parsed: SessionSecrets =
        serde_json::from_str(json).map_err(|err| AisecError::internal(err.to_string()))?;
    Ok((parsed.cookies, parsed.tokens))
}

pub fn resolve_auth_config_secrets(config: &mut AuthConfig, secrets: &SecretStore) -> AisecResult<()> {
    match config {
        AuthConfig::UsernamePassword {
            password,
            password_credential_id,
            ..
        } => {
            if password.is_none() {
                if let Some(id) = password_credential_id.clone() {
                    *password = Some(
                        secrets.load(SecretScope::Profile, &CredentialReferenceId::parse(id))?,
                    );
                }
            }
        }
        AuthConfig::Jwt {
            token,
            token_credential_id,
            ..
        } => {
            if token.is_empty() {
                if let Some(id) = token_credential_id.clone() {
                    *token = secrets.load(SecretScope::Profile, &CredentialReferenceId::parse(id))?;
                }
            }
        }
        AuthConfig::ApiKey {
            key,
            key_credential_id,
            ..
        } => {
            if key.is_empty() {
                if let Some(id) = key_credential_id.clone() {
                    *key = secrets.load(SecretScope::Profile, &CredentialReferenceId::parse(id))?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn store_auth_config_secrets(config: &mut AuthConfig, secrets: &SecretStore) -> AisecResult<()> {
    match config {
        AuthConfig::UsernamePassword {
            password,
            password_credential_id,
            ..
        } => {
            if let Some(value) = password.take() {
                let id = secrets.store(SecretScope::Profile, &value)?;
                *password_credential_id = Some(id.to_string());
            }
        }
        AuthConfig::Jwt {
            token,
            token_credential_id,
            ..
        } => {
            if !token.is_empty() {
                let id = secrets.store(SecretScope::Profile, token)?;
                *token = String::new();
                *token_credential_id = Some(id.to_string());
            }
        }
        AuthConfig::ApiKey {
            key,
            key_credential_id,
            ..
        } => {
            if !key.is_empty() {
                let id = secrets.store(SecretScope::Profile, key)?;
                *key = String::new();
                *key_credential_id = Some(id.to_string());
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aisec_storage::CreateAuthSessionRecord;

    #[tokio::test]
    async fn migrates_legacy_session_secrets() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let secrets = SecretStore::new().unwrap();

        let profile = db
            .repositories()
            .auth_profiles()
            .create(aisec_storage::CreateAuthProfile {
                project_id: None,
                name: "p".into(),
                method: "jwt".into(),
                config_json: serde_json::json!({}),
            })
            .await
            .unwrap();

        db.repositories()
            .auth_sessions()
            .create(CreateAuthSessionRecord {
                profile_id: profile.id,
                status: None,
                cookies_json: Some(serde_json::json!([{"name":"sid","value":"secret"}])),
                tokens_json: None,
                credential_reference_id: None,
                storage_state_path: None,
                expires_at: None,
                validation_status: None,
                user_identity: None,
            })
            .await
            .unwrap();

        let count = migrate_legacy_auth_data(&db, &secrets).await.unwrap();
        assert_eq!(count, 1);

        let legacy = db
            .repositories()
            .auth_sessions()
            .list_legacy_with_plaintext_secrets()
            .await
            .unwrap();
        assert!(legacy.is_empty());
    }
}
