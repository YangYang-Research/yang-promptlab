use promptlab_core::{PromptLabError, PromptLabResult};
use serde_json::{json, Value};

use super::store::{CredentialReferenceId, SecretScope, SecretStore};

const SECRET_KEYS: &[&str] = &["password", "token", "key", "value"];

/// Strip inline secrets from a target descriptor and store them in the OS keychain.
///
/// Returns `(sanitized_json, changed)`.
pub fn sanitize_target_descriptor(
    descriptor_json: &str,
    secrets: &SecretStore,
) -> PromptLabResult<(String, bool)> {
    let mut value: Value = serde_json::from_str(descriptor_json)
        .map_err(|err| PromptLabError::invalid_input(format!("invalid descriptor json: {err}")))?;
    let changed = sanitize_auth_block(value.get_mut("auth"), secrets)?;
    Ok((value.to_string(), changed))
}

/// Returns true when a descriptor JSON still contains inline auth secrets.
pub fn descriptor_has_plaintext_secrets(descriptor_json: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(descriptor_json) else {
        return false;
    };
    descriptor_value_has_plaintext(&value)
}

fn descriptor_value_has_plaintext(value: &Value) -> bool {
    let Some(auth) = value.get("auth") else {
        return false;
    };
    let Some(obj) = auth.as_object() else {
        return false;
    };

    if let Some(config) = obj.get("config").and_then(|v| v.as_object()) {
        for key in ["password", "token", "key", "value"] {
            if config
                .get(key)
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty())
            {
                return true;
            }
        }
    }

    for key in ["password", "token", "key", "value"] {
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

/// Resolve credential references inside a descriptor for runtime transport/auth use.
pub fn resolve_descriptor_for_runtime(
    descriptor_json: &str,
    secrets: &SecretStore,
) -> PromptLabResult<String> {
    let mut value: Value = serde_json::from_str(descriptor_json)
        .map_err(|err| PromptLabError::invalid_input(format!("invalid descriptor json: {err}")))?;
    resolve_auth_block(value.get_mut("auth"), secrets, false)?;
    Ok(value.to_string())
}

/// Resolve secrets for wizard edit forms. Missing keychain entries are dropped so the user can re-enter credentials.
pub fn resolve_descriptor_for_wizard(
    descriptor_json: &str,
    secrets: &SecretStore,
) -> PromptLabResult<String> {
    let mut value: Value = serde_json::from_str(descriptor_json)
        .map_err(|err| PromptLabError::invalid_input(format!("invalid descriptor json: {err}")))?;
    resolve_auth_block(value.get_mut("auth"), secrets, true)?;
    Ok(value.to_string())
}

fn sanitize_auth_block(auth: Option<&mut Value>, secrets: &SecretStore) -> PromptLabResult<bool> {
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

fn resolve_auth_block(
    auth: Option<&mut Value>,
    secrets: &SecretStore,
    lenient: bool,
) -> PromptLabResult<()> {
    let Some(auth_value) = auth else {
        return Ok(());
    };
    let Some(obj) = auth_value.as_object_mut() else {
        return Ok(());
    };

    if let Some(config) = obj.get_mut("config").and_then(|v| v.as_object_mut()) {
        resolve_secret_field(config, "password_credential_id", "password", secrets, lenient)?;
        resolve_secret_field(config, "token_credential_id", "token", secrets, lenient)?;
        resolve_secret_field(config, "key_credential_id", "key", secrets, lenient)?;
    }

    if let Some(id) = obj
        .get("credential_reference_id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
    {
        let secret = if lenient {
            secrets
                .load_optional(SecretScope::Target, &CredentialReferenceId::parse(id))?
        } else {
            Some(secrets.load(
                SecretScope::Target,
                &CredentialReferenceId::parse(id),
            )?)
        };
        if let Some(secret) = secret {
            if let Ok(parsed) = serde_json::from_str::<Value>(&secret) {
                if let Some(key) = parsed.get("key").and_then(|v| v.as_str()) {
                    if let Some(config) = obj.get_mut("config").and_then(|v| v.as_object_mut()) {
                        config.insert("key".into(), Value::String(key.to_string()));
                    }
                }
            }
        } else if lenient {
            obj.remove("credential_reference_id");
        }
    }

    Ok(())
}

fn sanitize_secret_field(
    obj: &mut serde_json::Map<String, Value>,
    secret_key: &str,
    ref_key: &str,
    secrets: &SecretStore,
) -> PromptLabResult<bool> {
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
    lenient: bool,
) -> PromptLabResult<()> {
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
    let credential_id = CredentialReferenceId::parse(id);
    let secret = if lenient {
        secrets.load_optional(SecretScope::Target, &credential_id)?
    } else {
        Some(secrets.load(SecretScope::Target, &credential_id)?)
    };
    match secret {
        Some(value) => {
            obj.insert(secret_key.into(), Value::String(value));
        }
        None if lenient => {
            tracing::debug!(
                ref_key,
                credential_id = %credential_id,
                "orphaned target credential reference removed for wizard"
            );
            obj.remove(ref_key);
            obj.insert(format!("{secret_key}_vault_missing"), json!(true));
        }
        None => {}
    }
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

    #[test]
    fn wizard_resolve_drops_orphaned_api_key_reference() {
        let store = secrets();
        let raw = r#"{"url":"https://api.example.com","auth":{"kind":"api_key","config":{"header_name":"Authorization","key_credential_id":"missing-id"}}}"#;
        let resolved = resolve_descriptor_for_wizard(raw, &store).unwrap();
        assert!(!resolved.contains("key_credential_id"));
        assert!(resolved.contains("key_vault_missing"));
    }
}
