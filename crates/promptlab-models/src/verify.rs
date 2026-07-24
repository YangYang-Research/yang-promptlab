use std::path::Path;

use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, BufReader};
use tokio::fs::File;
use tracing::debug;

use crate::error::{ModelError, ModelResult};
use crate::types::VerificationResult;

const CHUNK_SIZE: usize = 1024 * 1024;

/// Streaming SHA256 verification engine.
pub struct VerificationEngine;

impl VerificationEngine {
    /// Compute SHA256 hex digest for a file.
    pub async fn hash_file(path: impl AsRef<Path>) -> ModelResult<(String, u64)> {
        let path = path.as_ref();
        let mut file = BufReader::new(File::open(path).await?);
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; CHUNK_SIZE];
        let mut total = 0u64;

        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            total += n as u64;
        }

        Ok((hex_encode(hasher.finalize()), total))
    }

    /// Verify file against expected SHA256 (case-insensitive hex).
    pub async fn verify_file(
        path: impl AsRef<Path>,
        expected_sha256: Option<&str>,
    ) -> ModelResult<VerificationResult> {
        let path = path.as_ref();
        let (actual, size_bytes) = Self::hash_file(path).await?;

        let valid = match expected_sha256 {
            Some(expected) => normalize_hex(expected) == normalize_hex(&actual),
            None => true,
        };

        debug!(
            path = %path.display(),
            valid,
            size_bytes,
            "verification complete"
        );

        Ok(VerificationResult {
            file_path: path.to_path_buf(),
            expected_sha256: expected_sha256.map(str::to_string),
            actual_sha256: actual,
            size_bytes,
            valid,
        })
    }

    /// Verify and return error if mismatch.
    pub async fn verify_or_fail(
        path: impl AsRef<Path>,
        expected_sha256: &str,
    ) -> ModelResult<VerificationResult> {
        let result = Self::verify_file(path, Some(expected_sha256)).await?;
        if !result.valid {
            return Err(ModelError::verification(format!(
                "checksum mismatch: expected {}, got {}",
                normalize_hex(expected_sha256),
                result.actual_sha256
            )));
        }
        Ok(result)
    }
}

fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn normalize_hex(input: &str) -> String {
    input.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn verifies_known_hash() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.bin");
        tokio::fs::write(&path, b"hello world").await.unwrap();

        // SHA256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        let result = VerificationEngine::verify_file(&path, Some(expected))
            .await
            .unwrap();
        assert!(result.valid);
    }

    #[tokio::test]
    async fn detects_mismatch() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.bin");
        tokio::fs::write(&path, b"tampered").await.unwrap();

        let result = VerificationEngine::verify_file(&path, Some("00"))
            .await
            .unwrap();
        assert!(!result.valid);
    }
}
