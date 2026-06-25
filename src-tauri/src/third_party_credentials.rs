//! Third-party model credentials — registry metadata + encrypted local vault.

use std::path::Path;

use aisec_auth::{
    CredentialReferenceId, ModelCredentialVault, SecretScope, SecretStore,
};
use aisec_core::AisecError;
use aisec_models::{LocalModelManager, ModelProvider};
use tracing::info;

use crate::error::{CommandError, CommandResult};

pub const API_KEY_CREDENTIAL_ID: &str = "apiKeyCredentialId";
pub const AWS_SECRET_CREDENTIAL_ID: &str = "awsSecretAccessKeyCredentialId";
pub const AWS_SESSION_CREDENTIAL_ID: &str = "awsSessionTokenCredentialId";
pub const API_KEY_ENV: &str = "apiKeyEnv";
pub const LAST_CONNECTIVITY_OK: &str = "lastConnectivityOk";
pub const LAST_CONNECTIVITY: &str = "lastConnectivity";
pub const LAST_CONNECTIVITY_AT: &str = "lastConnectivityAt";
pub const CONNECTIVITY_SUCCESS: &str = "Connection Successful";
pub const CONNECTIVITY_FAILED: &str = "Connection Failed";

pub fn is_connectivity_success(value: &str) -> bool {
    value.starts_with(CONNECTIVITY_SUCCESS) || value.starts_with("Connected")
}

pub fn format_connectivity_value(ok: bool, latency_ms: u64) -> String {
    if ok {
        if latency_ms > 0 {
            format!("{CONNECTIVITY_SUCCESS} ({latency_ms} ms)")
        } else {
            CONNECTIVITY_SUCCESS.into()
        }
    } else {
        CONNECTIVITY_FAILED.into()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ThirdPartyCredentialFields {
    pub api_key: String,
    pub api_key_env: Option<String>,
    pub aws_secret_access_key: String,
    pub aws_session_token: String,
}

pub fn credential_id_from_metadata(metadata: &serde_json::Value, key: &str) -> Option<String> {
    metadata
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn copy_credential_metadata(from: &serde_json::Value, to: &mut serde_json::Value) {
    for key in [
        API_KEY_CREDENTIAL_ID,
        AWS_SECRET_CREDENTIAL_ID,
        AWS_SESSION_CREDENTIAL_ID,
        API_KEY_ENV,
    ] {
        if let Some(value) = from.get(key) {
            to[key] = value.clone();
        }
    }
}

pub fn has_new_credential_input(credentials: &ThirdPartyCredentialFields) -> bool {
    !credentials.api_key.trim().is_empty()
        || !credentials.aws_secret_access_key.trim().is_empty()
        || !credentials.aws_session_token.trim().is_empty()
}

pub fn persist_third_party_credentials(
    metadata: &mut serde_json::Value,
    credentials: &ThirdPartyCredentialFields,
    vault: &ModelCredentialVault,
) -> Result<(), CommandError> {
    if !credentials.api_key.trim().is_empty() {
        let id = vault
            .store(credentials.api_key.trim())
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
        metadata[API_KEY_CREDENTIAL_ID] = serde_json::Value::String(id.to_string());
    }
    if !credentials.aws_secret_access_key.trim().is_empty() {
        let id = vault
            .store(credentials.aws_secret_access_key.trim())
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
        metadata[AWS_SECRET_CREDENTIAL_ID] = serde_json::Value::String(id.to_string());
    }
    if !credentials.aws_session_token.trim().is_empty() {
        let id = vault
            .store(credentials.aws_session_token.trim())
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
        metadata[AWS_SESSION_CREDENTIAL_ID] = serde_json::Value::String(id.to_string());
    }
    if let Some(env) = credentials
        .api_key_env
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        metadata[API_KEY_ENV] = serde_json::Value::String(env.to_string());
    }
    Ok(())
}

pub fn resolve_third_party_credentials(
    credentials: &mut ThirdPartyCredentialFields,
    metadata: Option<&serde_json::Value>,
    vault: &ModelCredentialVault,
    secrets: &SecretStore,
) -> Result<(), CommandError> {
    let Some(metadata) = metadata else {
        return Ok(());
    };

    if credentials.api_key.trim().is_empty() {
        if let Some(id) = credential_id_from_metadata(metadata, API_KEY_CREDENTIAL_ID) {
            credentials.api_key = load_stored_secret(vault, secrets, &id, "API key")?;
        }
    }
    if credentials.aws_secret_access_key.trim().is_empty() {
        if let Some(id) = credential_id_from_metadata(metadata, AWS_SECRET_CREDENTIAL_ID) {
            credentials.aws_secret_access_key =
                load_stored_secret(vault, secrets, &id, "secret access key")?;
        }
    }
    if credentials.aws_session_token.trim().is_empty() {
        if let Some(id) = credential_id_from_metadata(metadata, AWS_SESSION_CREDENTIAL_ID) {
            credentials.aws_session_token =
                load_stored_secret(vault, secrets, &id, "session token")?;
        }
    }
    if credentials
        .api_key_env
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        credentials.api_key_env = credential_id_from_metadata(metadata, API_KEY_ENV);
    }
    Ok(())
}

pub fn validate_metadata_credentials(
    metadata: &serde_json::Value,
    vault: &ModelCredentialVault,
    secrets: &SecretStore,
) -> Result<(), CommandError> {
    let mut probe = ThirdPartyCredentialFields::default();
    resolve_third_party_credentials(&mut probe, Some(metadata), vault, secrets)
}

pub fn has_third_party_credentials_metadata(metadata: &serde_json::Value) -> bool {
    let has_api_key = credential_id_from_metadata(metadata, API_KEY_CREDENTIAL_ID).is_some()
        || metadata
            .get(API_KEY_ENV)
            .and_then(|value| value.as_str())
            .is_some_and(|value| !value.trim().is_empty());
    let has_secret = credential_id_from_metadata(metadata, AWS_SECRET_CREDENTIAL_ID).is_some();
    if has_secret {
        has_api_key
    } else {
        has_api_key
    }
}

pub fn apply_model_connectivity_metadata(
    metadata: &mut serde_json::Value,
    ok: bool,
    latency_ms: u64,
    checked_at: &str,
) {
    let connectivity = format_connectivity_value(ok, latency_ms);
    metadata[LAST_CONNECTIVITY_OK] = serde_json::Value::Bool(ok);
    metadata[LAST_CONNECTIVITY] = serde_json::Value::String(connectivity);
    metadata[LAST_CONNECTIVITY_AT] = serde_json::Value::String(checked_at.to_string());
}

pub fn connectivity_status_label(metadata: &serde_json::Value) -> Option<String> {
    let value = metadata.get(LAST_CONNECTIVITY)?.as_str()?;
    if is_connectivity_success(value) {
        Some(CONNECTIVITY_SUCCESS.into())
    } else if value == CONNECTIVITY_FAILED || value == "Failed" {
        Some(CONNECTIVITY_FAILED.into())
    } else {
        Some(value.to_string())
    }
}

pub fn short_connectivity_list_label(value: &str) -> Option<String> {
    connectivity_status_label(&serde_json::json!({ LAST_CONNECTIVITY: value }))
}

fn load_stored_secret(
    vault: &ModelCredentialVault,
    secrets: &SecretStore,
    id: &str,
    label: &str,
) -> Result<String, CommandError> {
    let reference = CredentialReferenceId::parse(id);
    vault
        .load(&reference)
        .or_else(|_| secrets.load(SecretScope::Model, &reference))
        .or_else(|_| secrets.load(SecretScope::Judge, &reference))
        .map_err(|_| {
            CommandError::invalid_input(format!(
                "stored {label} not found — re-enter credentials on the third-party model form and save again"
            ))
        })
}

fn rekey_credential_metadata(
    metadata: &mut serde_json::Value,
    vault: &ModelCredentialVault,
    secrets: &SecretStore,
    key: &str,
) -> Result<bool, CommandError> {
    let Some(old_id) = credential_id_from_metadata(metadata, key) else {
        return Ok(false);
    };
    let reference = CredentialReferenceId::parse(old_id);
    if vault.load(&reference).is_ok() {
        return Ok(false);
    }
    if secrets.load(SecretScope::Model, &reference).is_ok() {
        let secret = secrets
            .load(SecretScope::Model, &reference)
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
        let new_id = vault
            .store(secret.trim())
            .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
        metadata[key] = serde_json::Value::String(new_id.to_string());
        return Ok(true);
    }
    let secret = secrets
        .load(SecretScope::Judge, &reference)
        .map_err(|_| {
            CommandError::invalid_input(
                "legacy credential reference is invalid — re-enter API keys and save again",
            )
        })?;
    let new_id = vault
        .store(secret.trim())
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    metadata[key] = serde_json::Value::String(new_id.to_string());
    Ok(true)
}

/// One-time migration: move legacy keychain credentials into the encrypted model vault.
pub async fn migrate_third_party_model_credentials(
    data_dir: &Path,
    manager: &mut LocalModelManager,
    secrets: &SecretStore,
) -> CommandResult<u32> {
    let vault = ModelCredentialVault::new(data_dir)
        .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
    let remote_ids: Vec<String> = manager
        .list_models()
        .into_iter()
        .filter(|entry| entry.provider == ModelProvider::Remote)
        .map(|entry| entry.id.clone())
        .collect();

    let mut migrated = 0u32;
    for model_id in remote_ids {
        let Some(entry) = manager.get_model(&model_id).cloned() else {
            continue;
        };
        let mut metadata = entry.metadata.clone();
        let mut changed = rekey_credential_metadata(
            &mut metadata,
            &vault,
            secrets,
            API_KEY_CREDENTIAL_ID,
        )?;
        changed |= rekey_credential_metadata(
            &mut metadata,
            &vault,
            secrets,
            AWS_SECRET_CREDENTIAL_ID,
        )?;
        changed |= rekey_credential_metadata(
            &mut metadata,
            &vault,
            secrets,
            AWS_SESSION_CREDENTIAL_ID,
        )?;

        if changed {
            manager
                .update_model_metadata(&model_id, metadata)
                .map_err(|e| CommandError::from(AisecError::internal(e.to_string())))?;
            migrated += 1;
            info!(model_id = %model_id, "migrated third-party model credentials to encrypted vault");
        }
    }

    Ok(migrated)
}

pub fn open_model_credential_vault(data_dir: &Path) -> CommandResult<ModelCredentialVault> {
    ModelCredentialVault::new(data_dir).map_err(|e| CommandError::from(AisecError::internal(e.to_string())))
}
