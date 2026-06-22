use std::path::{Path, PathBuf};

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use aisec_core::{AisecError, AisecResult};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use rand::RngCore;
use sha2::{Digest, Sha256};

use super::store::CredentialReferenceId;

const NONCE_LEN: usize = 12;

/// Encrypted on-disk store for third-party model API credentials.
///
/// Uses a key derived from the app data directory so credentials survive restarts
/// without relying on macOS Keychain (which can fail read-after-write in unsigned dev builds).
pub struct ModelCredentialVault {
    cipher: Aes256Gcm,
    dir: PathBuf,
}

impl ModelCredentialVault {
    pub fn new(data_dir: &Path) -> AisecResult<Self> {
        let dir = data_dir.join("models/.credentials");
        std::fs::create_dir_all(&dir).map_err(AisecError::from)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        }

        let key = derive_key(data_dir);
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|err| AisecError::internal(format!("model credential cipher: {err}")))?;

        Ok(Self { cipher, dir })
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.enc"))
    }

    pub fn store(&self, secret: &str) -> AisecResult<CredentialReferenceId> {
        let id = CredentialReferenceId::new();
        self.store_with_id(&id, secret)?;
        Ok(id)
    }

    pub fn store_with_id(&self, id: &CredentialReferenceId, secret: &str) -> AisecResult<()> {
        let mut nonce = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), secret.as_bytes())
            .map_err(|err| AisecError::internal(format!("encrypt model credential: {err}")))?;

        let mut payload = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        payload.extend_from_slice(&nonce);
        payload.extend_from_slice(&ciphertext);

        let path = self.path(id.as_str());
        std::fs::write(&path, STANDARD.encode(payload)).map_err(AisecError::from)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub fn load(&self, id: &CredentialReferenceId) -> AisecResult<String> {
        let path = self.path(id.as_str());
        let encoded = std::fs::read_to_string(&path).map_err(AisecError::from)?;
        let payload = STANDARD
            .decode(encoded.trim())
            .map_err(|err| AisecError::internal(format!("decode model credential: {err}")))?;
        if payload.len() <= NONCE_LEN {
            return Err(AisecError::internal("model credential blob too short"));
        }
        let (nonce, ciphertext) = payload.split_at(NONCE_LEN);
        let plaintext = self
            .cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|err| AisecError::internal(format!("decrypt model credential: {err}")))?;
        String::from_utf8(plaintext).map_err(|err| AisecError::internal(err.to_string()))
    }

    pub fn delete(&self, id: &CredentialReferenceId) -> AisecResult<()> {
        match std::fs::remove_file(self.path(id.as_str())) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(AisecError::from(err)),
        }
    }
}

fn derive_key(data_dir: &Path) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"aisec-model-credential-vault-v1");
    hasher.update(data_dir.to_string_lossy().as_bytes());
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn roundtrip_model_credential() {
        let dir = tempdir().unwrap();
        let vault = ModelCredentialVault::new(dir.path()).unwrap();
        let id = vault.store("sk-test-secret").unwrap();
        assert_eq!(vault.load(&id).unwrap(), "sk-test-secret");
        vault.delete(&id).unwrap();
        assert!(vault.load(&id).is_err());
    }
}
