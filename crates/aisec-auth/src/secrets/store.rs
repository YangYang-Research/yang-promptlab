use std::fmt;

use aisec_core::{AisecError, AisecResult};
use uuid::Uuid;

/// Identifier for a secret stored outside SQLite (OS keychain / credential manager).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CredentialReferenceId(String);

impl CredentialReferenceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn parse(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CredentialReferenceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Logical namespace for keyring entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretScope {
    Session,
    Profile,
    Target,
    VaultKey,
    Judge,
    Model,
}

impl SecretScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Profile => "profile",
            Self::Target => "target",
            Self::VaultKey => "vault-key",
            Self::Judge => "judge",
            Self::Model => "model",
        }
    }
}

const SERVICE_NAME: &str = "com.aisec.app";

fn storage_key(scope: SecretScope, id: &str) -> String {
    format!("{}:{}", scope.as_str(), id)
}

#[cfg(test)]
fn test_backend() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>> {
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};
    static STORE: LazyLock<Mutex<HashMap<String, String>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    &STORE
}

/// OS secure storage (DPAPI / Keychain / Secret Service) via the `keyring` crate.
#[derive(Debug, Clone)]
pub struct SecretStore;

impl SecretStore {
    pub fn new() -> AisecResult<Self> {
        Ok(Self)
    }

    #[cfg(not(test))]
    fn entry(scope: SecretScope, id: &str) -> AisecResult<keyring::Entry> {
        keyring::Entry::new(SERVICE_NAME, &storage_key(scope, id))
            .map_err(|err| AisecError::internal(format!("keyring entry: {err}")))
    }

    pub fn store(&self, scope: SecretScope, secret: &str) -> AisecResult<CredentialReferenceId> {
        let id = CredentialReferenceId::new();
        self.store_with_id(scope, &id, secret)?;
        Ok(id)
    }

    pub fn store_with_id(
        &self,
        scope: SecretScope,
        id: &CredentialReferenceId,
        secret: &str,
    ) -> AisecResult<()> {
        #[cfg(test)]
        {
            test_backend()
                .lock()
                .expect("test secret store lock")
                .insert(storage_key(scope, id.as_str()), secret.to_string());
            return Ok(());
        }
        #[cfg(not(test))]
        Self::entry(scope, id.as_str())?
            .set_password(secret)
            .map_err(|err| AisecError::internal(format!("store secret: {err}")))
    }

    pub fn load(&self, scope: SecretScope, id: &CredentialReferenceId) -> AisecResult<String> {
        self.load_optional(scope, id)?
            .ok_or_else(|| AisecError::not_found("secret not found in secure storage"))
    }

    /// Load a secret when present; returns `None` if the keychain entry was removed or never existed.
    pub fn load_optional(
        &self,
        scope: SecretScope,
        id: &CredentialReferenceId,
    ) -> AisecResult<Option<String>> {
        #[cfg(test)]
        {
            return Ok(test_backend()
                .lock()
                .expect("test secret store lock")
                .get(&storage_key(scope, id.as_str()))
                .cloned());
        }
        #[cfg(not(test))]
        match Self::entry(scope, id.as_str())?.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(AisecError::internal(format!("load secret: {err}"))),
        }
    }

    pub fn delete(&self, scope: SecretScope, id: &CredentialReferenceId) -> AisecResult<()> {
        #[cfg(test)]
        {
            test_backend()
                .lock()
                .expect("test secret store lock")
                .remove(&storage_key(scope, id.as_str()));
            return Ok(());
        }
        #[cfg(not(test))]
        match Self::entry(scope, id.as_str())?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(AisecError::internal(format!("delete secret: {err}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_secret() {
        let store = SecretStore::new().unwrap();
        let payload = String::from("{\"cookies\":[]}");
        let id = store.store(SecretScope::Session, &payload).unwrap();
        let loaded = store.load(SecretScope::Session, &id).unwrap();
        assert_eq!(loaded, payload);
        store.delete(SecretScope::Session, &id).unwrap();
    }
}
