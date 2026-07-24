use std::path::{Path, PathBuf};

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use promptlab_core::{PromptLabError, PromptLabResult};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use rand::RngCore;
use tracing::debug;

use super::store::{CredentialReferenceId, SecretScope, SecretStore};

const MASTER_KEY_ID: &str = "master";
const NONCE_LEN: usize = 12;

/// Encrypts session artifacts at rest using a vault master key stored in the OS keychain.
#[derive(Clone)]
pub struct EncryptedVault {
    cipher: Aes256Gcm,
    vault_dir: PathBuf,
}

impl EncryptedVault {
    pub fn new(secrets: &SecretStore, vault_dir: impl Into<PathBuf>) -> PromptLabResult<Self> {
        let master_id = CredentialReferenceId::parse(MASTER_KEY_ID);
        let key_bytes = match secrets.load(SecretScope::VaultKey, &master_id) {
            Ok(existing) => decode_key(&existing)?,
            Err(_) => {
                let mut key = [0u8; 32];
                OsRng.fill_bytes(&mut key);
                secrets.store_with_id(
                    SecretScope::VaultKey,
                    &master_id,
                    &STANDARD.encode(key),
                )?;
                key.to_vec()
            }
        };

        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|err| PromptLabError::internal(format!("vault cipher: {err}")))?;

        Ok(Self {
            cipher,
            vault_dir: vault_dir.into(),
        })
    }

    pub fn vault_dir(&self) -> &Path {
        &self.vault_dir
    }

    pub fn encrypted_path(&self, session_id: &str) -> PathBuf {
        self.vault_dir.join(format!("{session_id}.storage.enc"))
    }

    pub async fn write_json(
        &self,
        session_id: &str,
        plaintext: &str,
    ) -> PromptLabResult<PathBuf> {
        tokio::fs::create_dir_all(&self.vault_dir)
            .await
            .map_err(PromptLabError::from)?;

        let path = self.encrypted_path(session_id);
        let mut nonce = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
            .map_err(|err| PromptLabError::internal(format!("encrypt vault artifact: {err}")))?;

        let mut payload = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        payload.extend_from_slice(&nonce);
        payload.extend_from_slice(&ciphertext);

        tokio::fs::write(&path, STANDARD.encode(payload))
            .await
            .map_err(PromptLabError::from)?;
        debug!(%session_id, path = %path.display(), "wrote encrypted session artifact");
        Ok(path)
    }

    pub async fn read_json(&self, path: &Path) -> PromptLabResult<String> {
        let encoded = tokio::fs::read_to_string(path)
            .await
            .map_err(PromptLabError::from)?;
        let payload = STANDARD
            .decode(encoded.trim())
            .map_err(|err| PromptLabError::internal(format!("decode vault artifact: {err}")))?;
        if payload.len() <= NONCE_LEN {
            return Err(PromptLabError::internal("vault artifact too short"));
        }
        let (nonce, ciphertext) = payload.split_at(NONCE_LEN);
        let plaintext = self
            .cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|err| PromptLabError::internal(format!("decrypt vault artifact: {err}")))?;
        String::from_utf8(plaintext).map_err(|err| PromptLabError::internal(err.to_string()))
    }

    pub async fn delete_artifact(&self, path: &Path) -> PromptLabResult<()> {
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(PromptLabError::from(err)),
        }
    }
}

fn decode_key(encoded: &str) -> PromptLabResult<Vec<u8>> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|err| PromptLabError::internal(format!("decode vault key: {err}")))?;
    if bytes.len() != 32 {
        return Err(PromptLabError::internal("invalid vault master key length"));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_vault() -> (SecretStore, EncryptedVault) {
        let secrets = SecretStore::new().unwrap();
        let dir = tempdir().unwrap();
        let vault = EncryptedVault::new(&secrets, dir.path()).unwrap();
        (secrets, vault)
    }

    #[tokio::test]
    async fn encrypts_and_decrypts_session_artifact() {
        let (_secrets, vault) = test_vault();
        let path = vault
            .write_json("sess-1", r#"{"cookies":[],"origins":[]}"#)
            .await
            .unwrap();
        let raw = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(!raw.contains("cookies"));
        let decrypted = vault.read_json(&path).await.unwrap();
        assert!(decrypted.contains("cookies"));
    }
}
