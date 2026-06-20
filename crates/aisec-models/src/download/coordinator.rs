use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use time::OffsetDateTime;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::download::DownloadManager;
use crate::error::{ModelError, ModelResult};
use crate::types::{DownloadProgress, DownloadStatus, HuggingFaceDownloadRequest};

/// Shared pause/cancel controls for an in-flight download.
#[derive(Clone)]
pub struct DownloadControl {
    pause: Arc<AtomicBool>,
    cancel: CancellationToken,
}

impl DownloadControl {
    pub fn new() -> Self {
        Self {
            pause: Arc::new(AtomicBool::new(false)),
            cancel: CancellationToken::new(),
        }
    }

    pub fn pause(&self) {
        self.pause.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.pause.store(false, Ordering::SeqCst);
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub(crate) async fn wait_if_paused(&self) -> ModelResult<()> {
        while self.pause.load(Ordering::SeqCst) {
            if self.cancel.is_cancelled() {
                return Err(ModelError::download("download cancelled"));
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        if self.cancel.is_cancelled() {
            return Err(ModelError::download("download cancelled"));
        }
        Ok(())
    }

    pub(crate) fn check_cancelled(&self) -> ModelResult<()> {
        if self.cancel.is_cancelled() {
            Err(ModelError::download("download cancelled"))
        } else {
            Ok(())
        }
    }
}

struct ActiveDownload {
    catalog_id: String,
    destination: PathBuf,
    control: DownloadControl,
    progress: Arc<Mutex<DownloadProgress>>,
    task: JoinHandle<()>,
}

/// Tracks a single active HuggingFace download with pause/resume/cancel.
pub struct DownloadCoordinator {
    downloader: DownloadManager,
    active: Mutex<Option<ActiveDownload>>,
}

impl DownloadCoordinator {
    pub fn new(downloader: DownloadManager) -> Self {
        Self {
            downloader,
            active: Mutex::new(None),
        }
    }

    pub async fn status(&self) -> Option<DownloadProgress> {
        let guard = self.active.lock().await;
        if let Some(active) = guard.as_ref() {
            return Some(active.progress.lock().await.clone());
        }
        None
    }

    pub async fn start_url_download(
        &self,
        catalog_id: impl Into<String>,
        url: &str,
        destination: PathBuf,
        expected_sha256: Option<String>,
        expected_size_bytes: Option<u64>,
    ) -> ModelResult<DownloadProgress> {
        let catalog_id = catalog_id.into();
        let mut guard = self.active.lock().await;
        // Replace a previous download that already finished (completed/failed) so the user can retry.
        let existing_terminal = match guard.as_ref() {
            Some(active) => matches!(
                active.progress.lock().await.status,
                DownloadStatus::Completed
                    | DownloadStatus::AwaitingVerify
                    | DownloadStatus::Failed
                    | DownloadStatus::VerifyFailed
            ),
            None => false,
        };
        if existing_terminal {
            if let Some(active) = guard.take() {
                active.task.abort();
            }
        }
        if guard.is_some() {
            return Err(ModelError::invalid("another download is already active"));
        }

        let control = DownloadControl::new();
        let progress = Arc::new(Mutex::new(DownloadProgress {
            model_id: catalog_id.clone(),
            status: DownloadStatus::Downloading,
            url: url.to_string(),
            destination: destination.clone(),
            downloaded_bytes: if destination.exists() {
                std::fs::metadata(&destination).map(|m| m.len()).unwrap_or(0)
            } else {
                0
            },
            total_bytes: expected_size_bytes,
            speed_bytes_per_sec: None,
            eta_seconds: None,
            resumed: destination.exists(),
            updated_at: OffsetDateTime::now_utc(),
            error: None,
        }));

        let downloader = self.downloader.clone();
        let control_clone = control.clone();
        let progress_task = progress.clone();
        let url_owned = url.to_string();
        let destination_for_task = destination.clone();
        let _expected_sha256 = expected_sha256;

        let task = tokio::spawn(async move {
            let result = downloader
                .download_controlled(
                    &url_owned,
                    &destination_for_task,
                    &control_clone,
                    progress_task.clone(),
                )
                .await;

            let mut slot = progress_task.lock().await;
            match result {
                Ok(done) => {
                    *slot = done;
                }
                Err(err) => {
                    warn!(error = %err, "controlled download failed");
                    let cancelled = control_clone.cancel.is_cancelled();
                    let paused = control_clone.pause.load(Ordering::SeqCst);
                    if !cancelled && paused {
                        slot.status = DownloadStatus::Paused;
                    } else {
                        slot.status = DownloadStatus::Failed;
                        slot.error = Some(err.to_string());
                        let state_path = destination_for_task.with_extension("download.json");
                        let _ = tokio::fs::remove_file(state_path).await;
                    }
                    slot.speed_bytes_per_sec = None;
                    slot.eta_seconds = None;
                    slot.updated_at = OffsetDateTime::now_utc();
                }
            }
        });

        *guard = Some(ActiveDownload {
            catalog_id,
            destination,
            control,
            progress: progress.clone(),
            task,
        });

        let snapshot = progress.lock().await.clone();
        Ok(snapshot)
    }

    pub async fn start_huggingface(
        &self,
        catalog_id: impl Into<String>,
        request: HuggingFaceDownloadRequest,
        destination: PathBuf,
    ) -> ModelResult<DownloadProgress> {
        let catalog_id = catalog_id.into();
        let mut guard = self.active.lock().await;
        // Replace a previous download that already finished (completed/failed) so the user can retry.
        let existing_terminal = match guard.as_ref() {
            Some(active) => matches!(
                active.progress.lock().await.status,
                DownloadStatus::Completed
                    | DownloadStatus::AwaitingVerify
                    | DownloadStatus::Failed
                    | DownloadStatus::VerifyFailed
            ),
            None => false,
        };
        if existing_terminal {
            if let Some(active) = guard.take() {
                active.task.abort();
            }
        }
        if guard.is_some() {
            return Err(ModelError::invalid("another download is already active"));
        }

        let control = DownloadControl::new();
        let progress = Arc::new(Mutex::new(DownloadProgress {
            model_id: catalog_id.clone(),
            status: DownloadStatus::Downloading,
            url: String::new(),
            destination: destination.clone(),
            downloaded_bytes: if destination.exists() {
                std::fs::metadata(&destination).map(|m| m.len()).unwrap_or(0)
            } else {
                0
            },
            total_bytes: request.expected_size_bytes,
            speed_bytes_per_sec: None,
            eta_seconds: None,
            resumed: destination.exists(),
            updated_at: OffsetDateTime::now_utc(),
            error: None,
        }));

        let downloader = self.downloader.clone();
        let control_clone = control.clone();
        let progress_task = progress.clone();
        let request_for_task = request.clone();
        let destination_for_task = destination.clone();

        let task = tokio::spawn(async move {
            let result = downloader
                .download_huggingface_controlled(
                    &request_for_task.repo,
                    &request_for_task.filename,
                    &destination_for_task,
                    request_for_task.revision.as_deref(),
                    &control_clone,
                    progress_task.clone(),
                )
                .await;

            let mut slot = progress_task.lock().await;
            match result {
                Ok(done) => {
                    *slot = done;
                }
                Err(err) => {
                    warn!(error = %err, "controlled download failed");
                    let cancelled = control_clone.cancel.is_cancelled();
                    let paused = control_clone.pause.load(Ordering::SeqCst);
                    if !cancelled && paused {
                        slot.status = DownloadStatus::Paused;
                    } else {
                        slot.status = DownloadStatus::Failed;
                        slot.error = Some(err.to_string());
                        let state_path = destination_for_task.with_extension("download.json");
                        let _ = tokio::fs::remove_file(state_path).await;
                    }
                    slot.speed_bytes_per_sec = None;
                    slot.eta_seconds = None;
                    slot.updated_at = OffsetDateTime::now_utc();
                }
            }
        });

        *guard = Some(ActiveDownload {
            catalog_id,
            destination,
            control,
            progress: progress.clone(),
            task,
        });

        let snapshot = progress.lock().await.clone();
        Ok(snapshot)
    }

    pub async fn pause(&self) -> ModelResult<DownloadProgress> {
        let guard = self.active.lock().await;
        let Some(active) = guard.as_ref() else {
            return Err(ModelError::not_found("active download"));
        };
        active.control.pause();
        let mut progress = active.progress.lock().await;
        progress.status = DownloadStatus::Paused;
        progress.speed_bytes_per_sec = None;
        progress.eta_seconds = None;
        progress.updated_at = OffsetDateTime::now_utc();
        Ok(progress.clone())
    }

    pub async fn resume(&self) -> ModelResult<DownloadProgress> {
        let guard = self.active.lock().await;
        let Some(active) = guard.as_ref() else {
            return Err(ModelError::not_found("active download"));
        };
        active.control.resume();
        let mut progress = active.progress.lock().await;
        progress.status = DownloadStatus::Downloading;
        progress.updated_at = OffsetDateTime::now_utc();
        Ok(progress.clone())
    }

    pub async fn cancel(&self) -> ModelResult<()> {
        let mut guard = self.active.lock().await;
        let Some(active) = guard.take() else {
            return Err(ModelError::not_found("active download"));
        };
        active.control.cancel();
        active.task.abort();
        let _ = tokio::fs::remove_file(&active.destination).await;
        let state_path = active.destination.with_extension("download.json");
        let _ = tokio::fs::remove_file(state_path).await;
        info!(catalog_id = %active.catalog_id, "download cancelled");
        Ok(())
    }

    fn promote_if_complete(
        progress: &mut DownloadProgress,
        task_finished: bool,
        destination: &Path,
    ) {
        if progress.status == DownloadStatus::Downloading && task_finished {
            progress.status = DownloadStatus::Completed;
            progress.speed_bytes_per_sec = None;
            progress.eta_seconds = Some(0);
        }

        if progress.status == DownloadStatus::Downloading {
            if let Ok(meta) = std::fs::metadata(destination) {
                let on_disk = meta.len();
                if on_disk > progress.downloaded_bytes {
                    progress.downloaded_bytes = on_disk;
                }
                if progress.total_bytes.is_some_and(|total| pipeline_download_complete(on_disk, total)) {
                    progress.status = DownloadStatus::Completed;
                    progress.speed_bytes_per_sec = None;
                    progress.eta_seconds = Some(0);
                }
            } else if progress
                .total_bytes
                .is_some_and(|total| pipeline_download_complete(progress.downloaded_bytes, total))
            {
                progress.status = DownloadStatus::Completed;
                progress.speed_bytes_per_sec = None;
                progress.eta_seconds = Some(0);
            }
        }
    }

    /// Completed download snapshot without removing the active slot (verify before consume).
    pub async fn snapshot_if_completed(&self) -> Option<(String, PathBuf, DownloadProgress)> {
        let guard = self.active.lock().await;
        let Some(active) = guard.as_ref() else {
            return None;
        };
        let current = active.progress.lock().await.clone();
        if current.status == DownloadStatus::Verifying {
            return None;
        }

        let mut progress = current;
        let task_finished = active.task.is_finished();
        Self::promote_if_complete(&mut progress, task_finished, &active.destination);

        if progress.status != DownloadStatus::Completed {
            return None;
        }

        Some((
            active.catalog_id.clone(),
            active.destination.clone(),
            progress,
        ))
    }

    /// Resume finalize when a prior poll already marked the job verifying.
    pub async fn snapshot_if_verifying(&self) -> Option<(String, PathBuf, DownloadProgress)> {
        let guard = self.active.lock().await;
        let Some(active) = guard.as_ref() else {
            return None;
        };
        let progress = active.progress.lock().await.clone();
        if progress.status != DownloadStatus::Verifying {
            return None;
        }
        Some((
            active.catalog_id.clone(),
            active.destination.clone(),
            progress,
        ))
    }

    pub async fn mark_verifying(&self) {
        let guard = self.active.lock().await;
        let Some(active) = guard.as_ref() else {
            return;
        };
        let mut slot = active.progress.lock().await;
        if matches!(
            slot.status,
            DownloadStatus::Completed
                | DownloadStatus::Downloading
                | DownloadStatus::AwaitingVerify
        ) {
            slot.status = DownloadStatus::Verifying;
            slot.speed_bytes_per_sec = None;
            slot.eta_seconds = None;
            slot.updated_at = OffsetDateTime::now_utc();
        }
    }

    pub async fn mark_awaiting_verify(&self) {
        let guard = self.active.lock().await;
        let Some(active) = guard.as_ref() else {
            return;
        };
        let mut slot = active.progress.lock().await;
        if matches!(
            slot.status,
            DownloadStatus::Verifying | DownloadStatus::Completed
        ) {
            slot.status = DownloadStatus::AwaitingVerify;
            slot.error = None;
            slot.speed_bytes_per_sec = None;
            slot.eta_seconds = Some(0);
            slot.updated_at = OffsetDateTime::now_utc();
        }
    }

    /// Remove the active slot after a successful finalize.
    pub async fn consume_completed(&self) {
        let mut guard = self.active.lock().await;
        let Some(active) = guard.as_ref() else {
            return;
        };
        let status = active.progress.lock().await.status;
        if !matches!(
            status,
            DownloadStatus::Completed
                | DownloadStatus::Verifying
                | DownloadStatus::AwaitingVerify
        ) {
            return;
        }
        if let Some(active) = guard.take() {
            active.task.abort();
        }
    }

    pub async fn take_if_completed(&self) -> Option<(String, PathBuf, DownloadProgress)> {
        let snapshot = self.snapshot_if_completed().await?;
        self.consume_completed().await;
        Some(snapshot)
    }

    pub async fn mark_verify_failed(&self, message: impl Into<String>) {
        let guard = self.active.lock().await;
        let Some(active) = guard.as_ref() else {
            return;
        };
        let mut slot = active.progress.lock().await;
        slot.status = DownloadStatus::VerifyFailed;
        slot.error = Some(message.into());
        slot.speed_bytes_per_sec = None;
        slot.eta_seconds = None;
        slot.updated_at = OffsetDateTime::now_utc();
    }

    /// Restore a finished download awaiting verification (no HTTP task).
    pub async fn restore_post_download(
        &self,
        catalog_id: impl Into<String>,
        destination: PathBuf,
        progress: DownloadProgress,
    ) -> ModelResult<DownloadProgress> {
        let catalog_id = catalog_id.into();
        let mut guard = self.active.lock().await;
        if let Some(active) = guard.as_ref() {
            let status = active.progress.lock().await.status;
            if matches!(
                status,
                DownloadStatus::Downloading | DownloadStatus::Paused
            ) {
                return Err(ModelError::invalid("another download is already active"));
            }
            if let Some(active) = guard.take() {
                active.task.abort();
            }
        }
        let progress = Arc::new(Mutex::new(progress));
        let snapshot = progress.lock().await.clone();
        let task = tokio::spawn(async {});
        *guard = Some(ActiveDownload {
            catalog_id,
            destination,
            control: DownloadControl::new(),
            progress,
            task,
        });
        Ok(snapshot)
    }

    /// Replace a post-download slot with a live HTTP resume task.
    pub async fn restart_url_download(
        &self,
        catalog_id: impl Into<String>,
        url: &str,
        destination: PathBuf,
        expected_sha256: Option<String>,
        expected_size_bytes: Option<u64>,
    ) -> ModelResult<DownloadProgress> {
        let mut guard = self.active.lock().await;
        if let Some(active) = guard.take() {
            active.control.cancel();
            active.task.abort();
        }
        drop(guard);
        self.start_url_download(
            catalog_id,
            url,
            destination,
            expected_sha256,
            expected_size_bytes,
        )
        .await
    }

    pub async fn mark_failed(&self, message: impl Into<String>) {
        let mut guard = self.active.lock().await;
        let Some(active) = guard.as_ref() else {
            return;
        };
        let mut slot = active.progress.lock().await;
        slot.status = DownloadStatus::Failed;
        slot.error = Some(message.into());
        slot.speed_bytes_per_sec = None;
        slot.eta_seconds = None;
        slot.updated_at = OffsetDateTime::now_utc();
    }

    pub async fn clear_if_finished(&self) {
        let mut guard = self.active.lock().await;
        let Some(active) = guard.as_ref() else {
            return;
        };
        let status = active.progress.lock().await.status;
        if matches!(
            status,
            DownloadStatus::Completed
                | DownloadStatus::Failed
                | DownloadStatus::VerifyFailed
                | DownloadStatus::AwaitingVerify
        ) {
            if let Some(active) = guard.take() {
                active.task.abort();
            }
        }
    }
}

fn download_near_complete(downloaded: u64, total: u64) -> bool {
    if total == 0 {
        return false;
    }
    let remaining = total.saturating_sub(downloaded);
    remaining <= 1024 || remaining * 100 <= total
}

fn pipeline_download_complete(on_disk: u64, total: u64) -> bool {
    total > 0 && on_disk + 1024 >= total
}
