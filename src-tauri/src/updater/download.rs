use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use tauri::AppHandle;
use url::Url;

use super::manifest::{validate_https_url, UpdateManifest};
use super::manifest::{parse_manifest, ResolvedAsset};
use super::{emit_progress, UpdateError, UpdateProgressDto, CURRENT_VERSION};

const MANIFEST_TIMEOUT: Duration = Duration::from_secs(15);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_INSTALLER_BYTES: u64 = 512 * 1024 * 1024;

pub async fn fetch_manifest(url: &str) -> Result<UpdateManifest, UpdateError> {
    let parsed = Url::parse(url).map_err(|err| UpdateError::UnsafeUrl(err.to_string()))?;
    validate_https_url(&parsed)?;
    let client = http_client(MANIFEST_TIMEOUT)?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|err| UpdateError::Network(format!("manifest request failed: {err}")))?;
    if !response.status().is_success() {
        return Err(UpdateError::Network(format!(
            "manifest HTTP {}",
            response.status()
        )));
    }
    let body = response
        .text()
        .await
        .map_err(|err| UpdateError::Network(format!("manifest read failed: {err}")))?;
    parse_manifest(&body)
}

pub async fn download_installer(
    app: &AppHandle,
    asset: &ResolvedAsset,
    dest_dir: &Path,
    latest_version: &str,
) -> Result<PathBuf, UpdateError> {
    let parsed = Url::parse(&asset.url).map_err(|err| UpdateError::UnsafeUrl(err.to_string()))?;
    validate_https_url(&parsed)?;

    let dest = dest_dir.join(&asset.filename);
    if dest.exists() {
        let _ = std::fs::remove_file(&dest);
    }

    let client = http_client(DOWNLOAD_TIMEOUT)?;
    let mut response = client
        .get(&asset.url)
        .send()
        .await
        .map_err(|err| UpdateError::Network(format!("download failed: {err}")))?;
    if !response.status().is_success() {
        return Err(UpdateError::Network(format!(
            "download HTTP {}",
            response.status()
        )));
    }

    let total = response
        .content_length()
        .or(asset.size)
        .filter(|size| *size > 0);
    if total.unwrap_or(0) > MAX_INSTALLER_BYTES {
        return Err(UpdateError::Network("installer exceeds 512 MiB limit".into()));
    }

    let tmp = dest.with_extension("partial");
    let mut file = std::fs::File::create(&tmp)
        .map_err(|err| UpdateError::Network(format!("cannot write installer: {err}")))?;
    let mut downloaded = 0u64;
    let mut last_emit = 0u64;

    loop {
        let chunk = response
            .chunk()
            .await
            .map_err(|err| UpdateError::Network(format!("download stream failed: {err}")))?;
        let Some(bytes) = chunk else {
            break;
        };
        downloaded = downloaded.saturating_add(bytes.len() as u64);
        if downloaded > MAX_INSTALLER_BYTES {
            let _ = std::fs::remove_file(&tmp);
            return Err(UpdateError::Network("installer exceeds 512 MiB limit".into()));
        }
        file.write_all(&bytes)
            .map_err(|err| UpdateError::Network(format!("cannot write installer: {err}")))?;
        if downloaded.saturating_sub(last_emit) >= 256 * 1024 || total == Some(downloaded) {
            last_emit = downloaded;
            emit_progress(
                app,
                UpdateProgressDto {
                    phase: "downloading".into(),
                    message: format!("Downloading PromptLab {latest_version}…"),
                    current_version: CURRENT_VERSION.into(),
                    latest_version: Some(latest_version.into()),
                    downloaded_bytes: Some(downloaded),
                    total_bytes: total,
                },
            );
        }
    }
    file.flush()
        .map_err(|err| UpdateError::Network(format!("cannot flush installer: {err}")))?;
    drop(file);
    std::fs::rename(&tmp, &dest)
        .map_err(|err| UpdateError::Network(format!("cannot finalize installer: {err}")))?;
    Ok(dest)
}

pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), UpdateError> {
    let data = std::fs::read(path)
        .map_err(|err| UpdateError::Network(format!("cannot read installer: {err}")))?;
    let actual = hex_encode(Sha256::digest(&data));
    let expected = expected.trim().to_ascii_lowercase();
    if actual != expected {
        let _ = std::fs::remove_file(path);
        return Err(UpdateError::Checksum { expected, actual });
    }
    Ok(())
}

fn http_client(timeout: Duration) -> Result<reqwest::Client, UpdateError> {
    promptlab_core::build_http_client(
        promptlab_core::HttpClientOptions::default()
            .with_timeout(timeout)
            .with_connect_timeout(CONNECT_TIMEOUT)
            .with_user_agent(format!("PromptLab/{CURRENT_VERSION}"))
            .with_redirect_limit(8),
    )
    .map_err(|err| UpdateError::Network(err.to_string()))
}

fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn verify_sha256_accepts_known_digest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("payload.bin");
        std::fs::write(&path, b"hello world").unwrap();
        let expected = hex_encode(Sha256::digest(b"hello world"));
        verify_sha256(&path, &expected).expect("hash matches");
    }

    #[test]
    fn verify_sha256_rejects_mismatch_and_deletes_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("payload.bin");
        std::fs::write(&path, b"tampered").unwrap();
        assert!(verify_sha256(&path, "00").is_err());
        assert!(!path.exists());
    }
}
