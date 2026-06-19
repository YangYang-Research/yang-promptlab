use std::path::{Path, PathBuf};
use std::sync::Arc;

use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE, RANGE};
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
            // Large GGUF files (4GB+) need hours on typical links.
            timeout_ms: 7_200_000,
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
            } else {
                downloaded = 0;
                Self::clear_state(destination).await?;
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

        let request = build_download_request(self.hf.client(), url, downloaded, self.options.timeout_ms);
        let response = request
            .send()
            .await
            .map_err(|e| map_download_http_error(url, e))?;

        downloaded = normalize_resume_start(&response, downloaded, &mut file, url).await?;
        validate_binary_response(&response, url).await?;

        let total_bytes = parse_content_length(&response, downloaded);
        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;

        while let Some(chunk) = stream
            .next()
            .await
            .transpose()
            .map_err(|e| map_download_stream_error(url, e))?
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
            speed_bytes_per_sec: None,
            eta_seconds: None,
            resumed,
            updated_at: OffsetDateTime::now_utc(),
            error: None,
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
            } else {
                downloaded = 0;
                Self::clear_state(destination).await?;
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

        let request = build_download_request(self.hf.client(), url, downloaded, self.options.timeout_ms);

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
            .map_err(|e| map_download_http_error(url, e))?;

        downloaded = normalize_resume_start(&response, downloaded, &mut file, url).await?;
        validate_binary_response(&response, url).await?;

        let total_bytes = parse_content_length(&response, downloaded);
        {
            let mut slot = progress_slot.lock().await;
            slot.total_bytes = total_bytes;
            slot.downloaded_bytes = downloaded;
        }

        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;

        let mut last_bytes = downloaded;
        let mut last_tick = std::time::Instant::now();

        while let Some(chunk) = stream
            .next()
            .await
            .transpose()
            .map_err(|e| map_download_stream_error(url, e))?
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

                let now = std::time::Instant::now();
                let elapsed = now.duration_since(last_tick).as_secs_f64();
                if elapsed >= 0.5 {
                    let delta = downloaded.saturating_sub(last_bytes);
                    let speed = delta as f64 / elapsed;
                    slot.speed_bytes_per_sec = Some(speed);
                    slot.eta_seconds = total_bytes.and_then(|total| {
                        let remaining = total.saturating_sub(downloaded);
                        if speed > 1.0 && remaining > 0 {
                            Some((remaining as f64 / speed).ceil() as u64)
                        } else {
                            None
                        }
                    });
                    last_bytes = downloaded;
                    last_tick = now;
                }
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
            speed_bytes_per_sec: None,
            eta_seconds: Some(0),
            resumed,
            updated_at: OffsetDateTime::now_utc(),
            error: None,
        };
        *progress_slot.lock().await = done.clone();
        Ok(done)
    }
}

fn build_download_request(
    client: &reqwest::Client,
    url: &str,
    downloaded: u64,
    timeout_ms: u64,
) -> reqwest::RequestBuilder {
    let mut request = client
        .get(url)
        .header(reqwest::header::ACCEPT_ENCODING, "identity");
    if timeout_ms > 0 {
        request = request.timeout(std::time::Duration::from_millis(timeout_ms));
    }
    if downloaded > 0 {
        request = request.header(RANGE, format!("bytes={downloaded}-"));
    }
    request
}

async fn normalize_resume_start(
    response: &reqwest::Response,
    downloaded: u64,
    file: &mut tokio::fs::File,
    url: &str,
) -> ModelResult<u64> {
    let status = response.status().as_u16();
    if status == 416 {
        file.set_len(0).await.map_err(ModelError::Io)?;
        file.seek(std::io::SeekFrom::Start(0)).await.map_err(ModelError::Io)?;
        return Err(ModelError::download(format!(
            "stale resume offset for {url}; cleared partial file — retry download"
        )));
    }

    if downloaded > 0 && status == 200 {
        info!(url, "server ignored Range request; restarting download from byte 0");
        file.set_len(0).await.map_err(ModelError::Io)?;
        file.seek(std::io::SeekFrom::Start(0)).await.map_err(ModelError::Io)?;
        return Ok(0);
    }

    if !(response.status().is_success() || status == 206) {
        return Err(ModelError::download(format!("HTTP {status} for {url}")));
    }

    Ok(downloaded)
}

async fn validate_binary_response(response: &reqwest::Response, url: &str) -> ModelResult<()> {
    if let Some(content_type) = response.headers().get(CONTENT_TYPE) {
        let Ok(value) = content_type.to_str() else {
            return Ok(());
        };
        let lower = value.to_ascii_lowercase();
        if lower.contains("json") || lower.contains("html") || lower.starts_with("text/") {
            return Err(ModelError::download(format!(
                "unexpected content-type '{value}' from {url}"
            )));
        }
    }
    Ok(())
}

fn map_download_http_error(url: &str, err: reqwest::Error) -> ModelError {
    if err.is_decode() {
        ModelError::download(format!(
            "compressed or invalid response from {url}; retry download ({err})"
        ))
    } else {
        ModelError::download(format!("{url}: {err}"))
    }
}

fn map_download_stream_error(url: &str, err: reqwest::Error) -> ModelError {
    if err.is_decode() {
        ModelError::download(format!(
            "stream decode failed for {url}; cancel, delete partial file, and retry ({err})"
        ))
    } else {
        ModelError::download(format!("{url}: {err}"))
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

    #[tokio::test]
    #[ignore = "network: HuggingFace GGUF download smoke test"]
    async fn downloads_mistral_catalog_url_smoke() {
        use reqwest::header::RANGE;

        use crate::download::HuggingFaceClient;

        let url = "https://huggingface.co/TheBloke/Mistral-7B-Instruct-v0.2-GGUF/resolve/main/mistral-7b-instruct-v0.2.Q4_K_M.gguf";
        let client = HuggingFaceClient::new();
        let response = client
            .client()
            .get(url)
            .header(RANGE, "bytes=0-4095")
            .send()
            .await
            .expect("request");
        assert!(
            response.status().is_success() || response.status().as_u16() == 206,
            "status {}",
            response.status()
        );
        let bytes = response.bytes().await.expect("read body");
        assert!(bytes.starts_with(b"GGUF"), "expected GGUF magic");
    }
}
