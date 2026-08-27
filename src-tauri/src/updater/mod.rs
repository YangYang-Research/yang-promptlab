//! PromptLab auto-update: fetch release `version.json`, download, install, relaunch.

mod download;
mod install;
mod manifest;

use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
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

/// Live update manifest: latest *published* GitHub Release asset `version.json`.
///
/// The release workflow uploads this file and undrafts the release so
/// `/releases/latest/download/version.json` resolves automatically.
pub const DEFAULT_MANIFEST_URL: &str =
    "https://github.com/YangYang-Research/yang-promptlab/releases/latest/download/version.json";

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

    let url = manifest_url();
    let manifest = match download::fetch_manifest(&url).await {
        Ok(manifest) => manifest,
        Err(err) => {
            warn!(error = %err, url = %url, "update manifest fetch failed");
            dto.skipped_reason = Some(err.to_string());
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
        dto.skipped_reason = Some("already on latest version".into());
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
    if resolve_asset(&manifest, &url, &platform).is_err() {
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

    let url = manifest_url();
    let manifest = download::fetch_manifest(&url).await.map_err(map_err)?;
    let platform = current_platform_key();
    let asset = resolve_asset(&manifest, &url, &platform).map_err(map_err)?;

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

    let downloaded = download::download_installer(app, &asset, &dest_dir, &manifest.version)
        .await
        .map_err(map_err)?;

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
    download::verify_sha256(&downloaded, &asset.sha256).map_err(map_err)?;

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
    CommandError::internal(err.to_string())
}

#[derive(Debug)]
pub enum UpdateError {
    Network(String),
    InvalidManifest(String),
    Checksum { expected: String, actual: String },
    Install(String),
    UnsafeUrl(String),
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
    }
}
