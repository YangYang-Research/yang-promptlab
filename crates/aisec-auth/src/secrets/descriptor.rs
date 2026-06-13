use aisec_core::{AisecError, AisecResult};
use serde_json::{json, Value};

use super::store::{CredentialReferenceId, SecretScope, SecretStore};

const SECRET_KEYS: &[&str] = &["password", "token", "key", "value"];

/// Strip inline secrets from a target descriptor and store them in the OS keychain.
///
/// Returns `(sanitized_json, changed)`.
pub fn sanitize_target_descriptor(
    descriptor_json: &str,
    secrets: &SecretStore,
) -> AisecResult<(String, bool)> {
    let mut value: Value = serde_json::from_str(descriptor_json)
        .map_err(|err| AisecError::invalid_input(format!("invalid descriptor json: {err}")))?;
    let mut changed = sanitize_auth_block(value.get_mut("auth"), secrets)?;
    Ok((value.to_string(), changed))
}

/// Resolve credential references inside a descriptor for runtime transport/auth use.
pub fn resolve_descriptor_for_runtime(
    descriptor_json: &str,
    secrets: &SecretStore,
) -> AisecResult<String> {
    let mut value: Value = serde_json::from_str(descriptor_json)
        .map_err(|err| AisecError::invalid_input(format!("invalid descriptor json: {err}")))?;
    resolve_auth_block(value.get_mut("auth"), secrets)?;
    Ok(value.to_string())
}

fn sanitize_auth_block(auth: Option<&mut Value>, secrets: &SecretStore) -> AisecResult<bool> {
    let Some(auth_value) = auth else {
        return Ok(false);
    };
    let Some(obj) = auth_value.as_object_mut() else {
        return Ok(false);
    };

    let mut changed = false;
    let config = obj
        .entry("config")
        .or_insert_with(|| json!({}))
        .as_object_mut();

    if let Some(config_obj) = config {
        changed |= sanitize_secret_field(config_obj, "password", "password_credential_id", secrets)?;
        changed |= sanitize_secret_field(config_obj, "token", "token_credential_id", secrets)?;
        changed |= sanitize_secret_field(config_obj, "key", "key_credential_id", secrets)?;
    }

    for key in SECRET_KEYS {
        if obj.contains_key(*key) {
            if let Some(raw) = obj.remove(*key).and_then(|v| v.as_str().map(str::to_string)) {
                let id = secrets.store(SecretScope::Target, &raw)?;
                obj.insert(
                    "credential_reference_id".into(),
                    Value::String(id.to_string()),
                );
                changed = true;
            }
        }
    }

    Ok(changed)
}

fn resolve_auth_block(auth: Option<&mut Value>, secrets: &SecretStore) -> AisecResult<()> {
    let Some(auth_value) = auth else {
        return Ok(());
    };
    let Some(obj) = auth_value.as_object_mut() else {
        return Ok(());
    };

    if let Some(config) = obj.get_mut("config").and_then(|v| v.as_object_mut()) {
        resolve_secret_field(config, "password_credential_id", "password", secrets)?;
        resolve_secret_field(config, "token_credential_id", "token", secrets)?;
        resolve_secret_field(config, "key_credential_id", "key", secrets)?;
    }

    if let Some(id) = obj
        .get("credential_reference_id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
    {
        let secret = secrets.load(SecretScope::Target, &CredentialReferenceId::parse(id))?;
        if let Ok(parsed) = serde_json::from_str::<Value>(&secret) {
            if let Some(key) = parsed.get("key").and_then(|v| v.as_str()) {
                if let Some(config) = obj.get_mut("config").and_then(|v| v.as_object_mut()) {
                    config.insert("key".into(), Value::String(key.to_string()));
                }
            }
        }
    }

    Ok(())
}

fn sanitize_secret_field(
    obj: &mut serde_json::Map<String, Value>,
    secret_key: &str,
    ref_key: &str,
    secrets: &SecretStore,
) -> AisecResult<bool> {
    if let Some(raw) = obj.remove(secret_key).and_then(|v| v.as_str().map(str::to_string)) {
        if raw.is_empty() {
            return Ok(false);
        }
        let id = secrets.store(SecretScope::Target, &raw)?;
        obj.insert(ref_key.into(), Value::String(id.to_string()));
        return Ok(true);
    }
    Ok(false)
}

fn resolve_secret_field(
    obj: &mut serde_json::Map<String, Value>,
    ref_key: &str,
    secret_key: &str,
    secrets: &SecretStore,
) -> AisecResult<()> {
    if obj.contains_key(secret_key) {
        return Ok(());
    }
    let Some(id) = obj
        .get(ref_key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
    else {
        return Ok(());
    };
    let secret = secrets.load(SecretScope::Target, &CredentialReferenceId::parse(id))?;
    obj.insert(secret_key.into(), Value::String(secret));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secrets() -> SecretStore {
        SecretStore::new().unwrap()
    }

    #[test]
    fn sanitizes_password_from_descriptor() {
        let store = secrets();
        let raw = r#"{"url":"https://app.example.com","auth":{"kind":"basic","config":{"username":"alice","password":"secret"}}}"#;
        let (sanitized, changed) = sanitize_target_descriptor(raw, &store).unwrap();
        assert!(changed);
        assert!(!sanitized.contains("secret"));
        assert!(sanitized.contains("password_credential_id"));
    }
}
