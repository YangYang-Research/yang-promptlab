use std::path::{Path, PathBuf};
use std::sync::Arc;

use reqwest::header::{CONTENT_LENGTH, RANGE};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::fs::{self, OpenOptions};
use tokio::sync::Mutex;
use tracing::{debug, info, instrument};

use crate::download::HuggingFaceClient;
use crate::download::coordinator::DownloadControl;
use crate::error::{ModelError, ModelResult};
use crate::types::{DownloadProgress, DownloadStatus};

/// Options controlling download behavior.
#[derive(Debug, Clone)]
pub struct DownloadOptions {
    pub chunk_size: usize,
    pub timeout_ms: u64,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            chunk_size: 1024 * 1024,
            timeout_ms: 300_000,
        }
    }
}

/// Resume state persisted alongside partial downloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResumeState {
    pub url: String,
    pub destination: PathBuf,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub updated_at: OffsetDateTime,
}

/// Resumable HTTP download manager.
#[derive(Clone)]
pub struct DownloadManager {
    hf: HuggingFaceClient,
    options: DownloadOptions,
}

impl DownloadManager {
    pub fn new(options: DownloadOptions) -> Self {
        Self {
            hf: HuggingFaceClient::new(),
            options,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(DownloadOptions::default())
    }

    fn state_path(destination: &Path) -> PathBuf {
        destination.with_extension("download.json")
    }

    async fn load_state(destination: &Path) -> ModelResult<Option<ResumeState>> {
        let path = Self::state_path(destination);
        if !path.exists() {
            return Ok(None);
        }
        let data = fs::read_to_string(&path).await?;
        let state: ResumeState = serde_json::from_str(&data)
            .map_err(|e| ModelError::download(format!("invalid resume state: {e}")))?;
        Ok(Some(state))
    }

    async fn save_state(state: &ResumeState) -> ModelResult<()> {
        if let Some(parent) = state.destination.parent() {
            fs::create_dir_all(parent).await?;
        }
        let data = serde_json::to_string_pretty(state)
            .map_err(|e| ModelError::download(e.to_string()))?;
        fs::write(Self::state_path(&state.destination), data).await?;
        Ok(())
    }

    async fn clear_state(destination: &Path) -> ModelResult<()> {
        let path = Self::state_path(destination);
        if path.exists() {
            fs::remove_file(path).await?;
        }
        Ok(())
    }

    /// Download a file with HTTP Range resume support.
    #[instrument(skip(self, destination), fields(url = %url))]
    pub async fn download(
        &self,
        url: &str,
        destination: impl AsRef<Path>,
    ) -> ModelResult<crate::types::DownloadProgress> {
        let destination = destination.as_ref();
        let existing = Self::load_state(destination).await?;
        let mut downloaded = if destination.exists() {
            fs::metadata(destination).await.map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        if let Some(state) = &existing {
            if state.url == url {
                downloaded = downloaded.max(state.downloaded_bytes);
            }
        }

        let resumed = downloaded > 0;
        if resumed {
            info!(downloaded, "resuming download");
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(destination)
            .await?;

        if downloaded > 0 {
            file.seek(std::io::SeekFrom::Start(downloaded)).await?;
        }

        let timeout = std::time::Duration::from_millis(self.options.timeout_ms);
        let mut request = self.hf.client().get(url).timeout(timeout);
        if downloaded > 0 {
            request = request.header(RANGE, format!("bytes={downloaded}-"));
        }

        let response = request
            .send()
            .await
            .map_err(|e| ModelError::download(e.to_string()))?;

        if !response.status().is_success() && response.status().as_u16() != 206 {
            return Err(ModelError::download(format!(
                "HTTP {} for {}",
                response.status(),
                url
            )));
        }

        let total_bytes = parse_content_length(&response, downloaded);
        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;

        while let Some(chunk) = stream
            .next()
            .await
            .transpose()
            .map_err(|e| ModelError::download(e.to_string()))?
        {
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            Self::save_state(&ResumeState {
                url: url.to_string(),
                destination: destination.to_path_buf(),
                downloaded_bytes: downloaded,
                total_bytes,
                updated_at: OffsetDateTime::now_utc(),
            })
            .await?;
        }

        file.flush().await?;
        Self::clear_state(destination).await?;

        debug!(downloaded, "download complete");

        Ok(crate::types::DownloadProgress {
            model_id: String::new(),
            status: DownloadStatus::Completed,
            url: url.to_string(),
            destination: destination.to_path_buf(),
            downloaded_bytes: downloaded,
            total_bytes,
            resumed,
            updated_at: OffsetDateTime::now_utc(),
        })
    }

    /// Download from HuggingFace resolve URL.
    pub async fn download_huggingface(
        &self,
        repo: &str,
        filename: &str,
        destination: impl AsRef<Path>,
        revision: Option<&str>,
    ) -> ModelResult<crate::types::DownloadProgress> {
        let url = self.hf.resolve_url(repo, filename, revision);
        self.download(&url, destination).await
    }

    /// Download from HuggingFace with pause/cancel controls and live progress updates.
    pub async fn download_huggingface_controlled(
        &self,
        repo: &str,
        filename: &str,
        destination: impl AsRef<Path>,
        revision: Option<&str>,
        control: &DownloadControl,
        progress: Arc<Mutex<DownloadProgress>>,
    ) -> ModelResult<DownloadProgress> {
        let url = self.hf.resolve_url(repo, filename, revision);
        self.download_controlled(&url, destination, control, progress)
            .await
    }

    /// Download with pause/cancel controls and live progress updates.
    pub async fn download_controlled(
        &self,
        url: &str,
        destination: impl AsRef<Path>,
        control: &DownloadControl,
        progress_slot: Arc<Mutex<DownloadProgress>>,
    ) -> ModelResult<DownloadProgress> {
        let destination = destination.as_ref();
        let existing = Self::load_state(destination).await?;
        let mut downloaded = if destination.exists() {
            fs::metadata(destination).await.map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        if let Some(state) = &existing {
            if state.url == url {
                downloaded = downloaded.max(state.downloaded_bytes);
            }
        }

        let resumed = downloaded > 0;
        if resumed {
            info!(downloaded, "resuming download");
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(destination)
            .await?;

        if downloaded > 0 {
            file.seek(std::io::SeekFrom::Start(downloaded)).await?;
        }

        let timeout = std::time::Duration::from_millis(self.options.timeout_ms);
        let mut request = self.hf.client().get(url).timeout(timeout);
        if downloaded > 0 {
            request = request.header(RANGE, format!("bytes={downloaded}-"));
        }

        {
            let mut slot = progress_slot.lock().await;
            slot.url = url.to_string();
            slot.destination = destination.to_path_buf();
            slot.downloaded_bytes = downloaded;
            slot.resumed = resumed;
            slot.status = DownloadStatus::Downloading;
            slot.updated_at = OffsetDateTime::now_utc();
        }

        let response = request
            .send()
            .await
            .map_err(|e| ModelError::download(e.to_string()))?;

        if !response.status().is_success() && response.status().as_u16() != 206 {
            return Err(ModelError::download(format!(
                "HTTP {} for {}",
                response.status(),
                url
            )));
        }

        let total_bytes = parse_content_length(&response, downloaded);
        {
            let mut slot = progress_slot.lock().await;
            slot.total_bytes = total_bytes;
        }

        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;

        while let Some(chunk) = stream
            .next()
            .await
            .transpose()
            .map_err(|e| ModelError::download(e.to_string()))?
        {
            control.wait_if_paused().await?;
            control.check_cancelled()?;

            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            {
                let mut slot = progress_slot.lock().await;
                slot.downloaded_bytes = downloaded;
                slot.total_bytes = total_bytes;
                slot.status = DownloadStatus::Downloading;
                slot.updated_at = OffsetDateTime::now_utc();
            }

            Self::save_state(&ResumeState {
                url: url.to_string(),
                destination: destination.to_path_buf(),
                downloaded_bytes: downloaded,
                total_bytes,
                updated_at: OffsetDateTime::now_utc(),
            })
            .await?;
        }

        file.flush().await?;
        Self::clear_state(destination).await?;

        let done = DownloadProgress {
            model_id: progress_slot.lock().await.model_id.clone(),
            status: DownloadStatus::Completed,
            url: url.to_string(),
            destination: destination.to_path_buf(),
            downloaded_bytes: downloaded,
            total_bytes,
            resumed,
            updated_at: OffsetDateTime::now_utc(),
        };
        *progress_slot.lock().await = done.clone();
        Ok(done)
    }
}

fn parse_content_length(response: &reqwest::Response, offset: u64) -> Option<u64> {
    if response.status().as_u16() == 206 {
        if let Some(range) = response.headers().get("content-range") {
            if let Ok(s) = range.to_str() {
                // bytes 0-999/1000
                if let Some(total) = s.split('/').nth(1) {
                    return total.parse().ok();
                }
            }
        }
    }

    response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(|len| if response.status().as_u16() == 206 {
            offset + len
        } else {
            len
        })
}

// futures_util for stream - add to Cargo.toml

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn downloads_full_file() {
        let server = MockServer::start().await;
        let body = b"gguf-model-data-chunk";

        Mock::given(method("GET"))
            .and(path("/model.gguf"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("model.gguf");
        let mgr = DownloadManager::with_defaults();
        let progress = mgr
            .download(&format!("{}/model.gguf", server.uri()), &dest)
            .await
            .unwrap();

        assert_eq!(progress.status, DownloadStatus::Completed);
        assert_eq!(fs::read(&dest).await.unwrap(), body);
    }

    #[tokio::test]
    async fn resumes_partial_download() {
        let server = MockServer::start().await;
        let body = b"0123456789abcdef";

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("resume.gguf");
        let url = format!("{}/resume.gguf", server.uri());

        // Simulate partial download (first 8 bytes)
        fs::write(&dest, &body[..8]).await.unwrap();
        fs::write(
            DownloadManager::state_path(&dest),
            serde_json::to_string(&ResumeState {
                url: url.clone(),
                destination: dest.clone(),
                downloaded_bytes: 8,
                total_bytes: Some(body.len() as u64),
                updated_at: OffsetDateTime::now_utc(),
            })
            .unwrap(),
        )
        .await
        .unwrap();

        Mock::given(method("GET"))
            .and(path("/resume.gguf"))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("content-range", "bytes 8-15/16")
                    .set_body_bytes(&body[8..]),
            )
            .mount(&server)
            .await;

        let mgr = DownloadManager::with_defaults();
        let progress = mgr.download(&url, &dest).await.unwrap();

        assert!(progress.resumed);
        assert_eq!(progress.downloaded_bytes, body.len() as u64);
        assert_eq!(fs::read(&dest).await.unwrap(), body);
    }
}
