//! PromptLab auto-update: fetch release `version.json`, download, install, relaunch.

mod download;
mod install;
mod manifest;

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tracing::{info, warn};

pub use manifest::{
    current_platform_key, is_newer_version, parse_manifest, resolve_asset, UpdateManifest,
    UpdatePlatformAsset,
};

use crate::error::{CommandError, CommandResult};

pub const UPDATE_PROGRESS_EVENT: &str = "app-update-progress";
pub const SKIP_UPDATE_ENV: &str = "PROMPTLAB_SKIP_UPDATE";
pub const FORCE_UPDATE_ENV: &str = "PROMPTLAB_FORCE_UPDATE";
pub const MANIFEST_URL_ENV: &str = "PROMPTLAB_UPDATE_MANIFEST_URL";

/// Primary update manifest (GitHub Release `version.json`).
///
/// Release workflow uploads this file and undrafts so
/// `/releases/latest/download/version.json` resolves.
pub const DEFAULT_MANIFEST_URL: &str =
    "https://github.com/YangYang-Research/yang-promptlab/releases/latest/download/version.json";

/// CDN backup on yangyang R2 public bucket (`yangyang-org-public`).
pub const DEFAULT_MANIFEST_BACKUP_URL: &str =
    "https://cdn.yyng.icu/products/yang-promptlab/manifests/version.json";

pub const MANIFEST_BACKUP_URL_ENV: &str = "PROMPTLAB_UPDATE_MANIFEST_BACKUP_URL";

pub(crate) const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckDto {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub platform: String,
    pub update_available: bool,
    pub applied: bool,
    pub notes: Option<String>,
    pub skipped_reason: Option<String>,
}

impl UpdateCheckDto {
    fn current() -> Self {
        Self {
            current_version: CURRENT_VERSION.into(),
            latest_version: None,
            platform: current_platform_key(),
            update_available: false,
            applied: false,
            notes: None,
            skipped_reason: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgressDto {
    pub phase: String,
    pub message: String,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
}

pub fn emit_progress(app: &AppHandle, event: UpdateProgressDto) {
    let _ = app.emit(UPDATE_PROGRESS_EVENT, event);
}

pub fn manifest_url() -> String {
    std::env::var(MANIFEST_URL_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_MANIFEST_URL.into())
}

pub fn manifest_backup_url() -> Option<String> {
    std::env::var(MANIFEST_BACKUP_URL_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| Some(DEFAULT_MANIFEST_BACKUP_URL.into()))
        .filter(|backup| backup != &manifest_url())
}

/// GitHub primary, then CDN backup (if distinct).
pub fn manifest_urls() -> Vec<String> {
    let mut urls = vec![manifest_url()];
    if let Some(backup) = manifest_backup_url() {
        urls.push(backup);
    }
    urls
}

pub fn auto_apply_blocked() -> Option<String> {
    if env_flag(SKIP_UPDATE_ENV) {
        return Some("updates disabled (PROMPTLAB_SKIP_UPDATE)".into());
    }
    if cfg!(debug_assertions) && !env_flag(FORCE_UPDATE_ENV) {
        return Some("skipped in debug builds".into());
    }
    None
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref().map(str::trim),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn cache_dir() -> PathBuf {
    promptlab_core::environment::default_root_dir()
        .join("cache")
        .join("updates")
}

fn pending_success_path() -> PathBuf {
    cache_dir().join("pending-success.json")
}

/// Written just before relaunch; consumed once on the next boot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSuccessMarker {
    pub from_version: String,
    pub to_version: String,
}

pub fn mark_update_pending(from_version: &str, to_version: &str) {
    let dir = cache_dir();
    if let Err(err) = std::fs::create_dir_all(&dir) {
        warn!(error = %err, "failed to create update cache for success marker");
        return;
    }
    let marker = UpdateSuccessMarker {
        from_version: from_version.to_string(),
        to_version: to_version.to_string(),
    };
    match serde_json::to_vec_pretty(&marker) {
        Ok(bytes) => {
            if let Err(err) = std::fs::write(pending_success_path(), bytes) {
                warn!(error = %err, "failed to write update success marker");
            }
        }
        Err(err) => warn!(error = %err, "failed to serialize update success marker"),
    }
}

/// Returns marker when `to_version` matches this binary; deletes file either way if present.
/// Cached per process so StrictMode / double `app_info` still sees the toast once.
pub fn consume_update_success() -> Option<UpdateSuccessMarker> {
    static CONSUMED: OnceLock<Option<UpdateSuccessMarker>> = OnceLock::new();
    CONSUMED
        .get_or_init(|| {
            let path = pending_success_path();
            let raw = match std::fs::read_to_string(&path) {
                Ok(raw) => raw,
                Err(_) => return None,
            };
            let _ = std::fs::remove_file(&path);
            let marker: UpdateSuccessMarker = match serde_json::from_str(&raw) {
                Ok(m) => m,
                Err(err) => {
                    warn!(error = %err, "invalid update success marker");
                    return None;
                }
            };
            if marker.to_version.trim() == CURRENT_VERSION {
                Some(marker)
            } else {
                warn!(
                    marker_to = %marker.to_version,
                    current = CURRENT_VERSION,
                    "ignoring update success marker for different version"
                );
                None
            }
        })
        .clone()
}

fn progress(
    phase: &str,
    message: impl Into<String>,
    latest: Option<&str>,
    downloaded: Option<u64>,
    total: Option<u64>,
) -> UpdateProgressDto {
    UpdateProgressDto {
        phase: phase.into(),
        message: message.into(),
        current_version: CURRENT_VERSION.into(),
        latest_version: latest.map(str::to_string),
        downloaded_bytes: downloaded,
        total_bytes: total,
    }
}

/// Fetch the remote manifest and compare against the running binary.
pub async fn check_for_update(app: Option<&AppHandle>) -> CommandResult<UpdateCheckDto> {
    let mut dto = UpdateCheckDto::current();
    if let Some(app) = app {
        emit_progress(
            app,
            progress("checking", "Checking for updates…", None, None, None),
        );
    }

    let urls = manifest_urls();
    let primary = urls.first().cloned().unwrap_or_default();
    let manifest = match download::fetch_manifest(&urls).await {
        Ok(manifest) => manifest,
        Err(err) => {
            warn!(error = %err, "update manifest fetch failed on all sources");
            // Keep technical detail in logs; UI toast uses skipped_reason (once).
            dto.skipped_reason = Some(err.user_message());
            if let Some(app) = app {
                emit_progress(
                    app,
                    progress("idle", "No update available.", None, None, None),
                );
            }
            return Ok(dto);
        }
    };

    dto.latest_version = Some(manifest.version.clone());
    dto.notes = if manifest.notes.trim().is_empty() {
        None
    } else {
        Some(manifest.notes.clone())
    };

    if !is_newer_version(&manifest.version, CURRENT_VERSION) {
        dto.skipped_reason = Some("Already on latest version".into());
        if let Some(app) = app {
            emit_progress(
                app,
                progress(
                    "idle",
                    "PromptLab is up to date.",
                    Some(&manifest.version),
                    None,
                    None,
                ),
            );
        }
        return Ok(dto);
    }

    let platform = current_platform_key();
    if resolve_asset(&manifest, &primary, &platform).is_err() {
        dto.skipped_reason = Some(format!("no installer for platform {platform}"));
        return Ok(dto);
    }

    dto.update_available = true;
    if let Some(app) = app {
        emit_progress(
            app,
            progress(
                "available",
                format!("Update {} is available.", manifest.version),
                Some(&manifest.version),
                None,
                None,
            ),
        );
    }
    Ok(dto)
}

/// Check, and if a newer signed build exists, download → install → relaunch.
pub async fn apply_if_available(app: &AppHandle) -> CommandResult<UpdateCheckDto> {
    if let Some(reason) = auto_apply_blocked() {
        let mut dto = UpdateCheckDto::current();
        dto.skipped_reason = Some(reason);
        return Ok(dto);
    }

    let mut dto = check_for_update(Some(app)).await?;
    if !dto.update_available {
        return Ok(dto);
    }

    let urls = manifest_urls();
    let primary = urls.first().cloned().unwrap_or_default();
    let manifest = download::fetch_manifest(&urls).await.map_err(map_err)?;
    let platform = current_platform_key();
    let asset = resolve_asset(&manifest, &primary, &platform).map_err(map_err)?;

    if asset.sha256.trim().is_empty() {
        dto.skipped_reason = Some("update skipped: installer sha256 is missing".into());
        dto.update_available = true;
        warn!("refusing to install update without sha256");
        return Ok(dto);
    }

    let dest_dir = cache_dir();
    std::fs::create_dir_all(&dest_dir).map_err(|err| {
        CommandError::internal(format!("failed to create update cache: {err}"))
    })?;

    emit_progress(
        app,
        progress(
            "downloading",
            format!("Downloading PromptLab {}…", manifest.version),
            Some(&manifest.version),
            Some(0),
            asset.size.filter(|size| *size > 0),
        ),
    );

    let downloaded = match download::download_installer(app, &asset, &dest_dir, &manifest.version)
        .await
    {
        Ok(path) => path,
        Err(err) => {
            emit_progress(
                app,
                progress(
                    "error",
                    err.user_message(),
                    Some(&manifest.version),
                    None,
                    None,
                ),
            );
            return Err(map_err(err));
        }
    };

    emit_progress(
        app,
        progress(
            "verifying",
            "Verifying installer checksum…",
            Some(&manifest.version),
            None,
            None,
        ),
    );
    if let Err(err) = download::verify_sha256(&downloaded, &asset.sha256) {
        emit_progress(
            app,
            progress(
                "error",
                err.user_message(),
                Some(&manifest.version),
                None,
                None,
            ),
        );
        return Err(map_err(err));
    }

    emit_progress(
        app,
        progress(
            "installing",
            format!("Installing PromptLab {}…", manifest.version),
            Some(&manifest.version),
            None,
            None,
        ),
    );

    let launch = install::install_and_prepare_launch(&downloaded).map_err(map_err)?;

    emit_progress(
        app,
        progress(
            "relaunching",
            "Restarting the new version…",
            Some(&manifest.version),
            None,
            None,
        ),
    );

    let pid = std::process::id();
    install::spawn_after_exit(pid, &launch).map_err(map_err)?;

    mark_update_pending(CURRENT_VERSION, &manifest.version);

    info!(
        version = %manifest.version,
        platform = %platform,
        "update installed; current process will exit"
    );

    dto.applied = true;
    dto.latest_version = Some(manifest.version);
    dto.skipped_reason = None;

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(400)).await;
        app_handle.exit(0);
    });

    Ok(dto)
}

fn map_err(err: UpdateError) -> CommandError {
    CommandError::internal(err.user_message())
}

#[derive(Debug)]
pub enum UpdateError {
    Network(String),
    InvalidManifest(String),
    Checksum { expected: String, actual: String },
    Install(String),
    UnsafeUrl(String),
}

impl UpdateError {
    /// Short UI copy — never leak URLs or reqwest internals for network faults.
    pub fn user_message(&self) -> String {
        match self {
            Self::Network(_) => {
                "Update failed due to a network error. Check your connection and try again.".into()
            }
            Self::InvalidManifest(_) => "Update manifest is invalid. Try again later.".into(),
            Self::Checksum { .. } => {
                "Update file failed verification. Try downloading again.".into()
            }
            Self::Install(msg) => format!("Update install failed: {msg}"),
            Self::UnsafeUrl(_) => "Update source was rejected.".into(),
        }
    }
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(msg) => write!(f, "{msg}"),
            Self::InvalidManifest(msg) => write!(f, "invalid update manifest: {msg}"),
            Self::Checksum { expected, actual } => {
                write!(f, "installer checksum mismatch (expected {expected}, got {actual})")
            }
            Self::Install(msg) => write!(f, "update install failed: {msg}"),
            Self::UnsafeUrl(msg) => write!(f, "rejected update URL: {msg}"),
        }
    }
}

impl std::error::Error for UpdateError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_manifest_url_is_https() {
        assert!(DEFAULT_MANIFEST_URL.starts_with("https://"));
        assert!(DEFAULT_MANIFEST_URL.contains("/releases/latest/download/"));
        assert!(DEFAULT_MANIFEST_URL.ends_with("/version.json"));
        assert!(DEFAULT_MANIFEST_BACKUP_URL.starts_with("https://cdn.yyng.icu/"));
        assert!(DEFAULT_MANIFEST_BACKUP_URL.contains("/products/yang-promptlab/manifests/"));
    }

    #[test]
    fn manifest_urls_primary_then_backup() {
        let urls = manifest_urls();
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], DEFAULT_MANIFEST_URL);
        assert_eq!(urls[1], DEFAULT_MANIFEST_BACKUP_URL);
    }

    #[test]
    fn network_user_message_hides_url() {
        let err = UpdateError::Network(
            "manifest request failed: error sending request for url (https://cdn.yyng.icu/x)".into(),
        );
        let msg = err.user_message();
        assert!(!msg.contains("http"));
        assert!(msg.to_ascii_lowercase().contains("network"));
    }
}
