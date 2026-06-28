//! Tauri event payloads for live UI updates.

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use time::OffsetDateTime;

pub const SCAN_PROGRESS_EVENT: &str = "scan-progress";
pub const APP_DATA_CHANGED_EVENT: &str = "app-data-changed";
pub const RUNTIME_INSTALL_PROGRESS_EVENT: &str = "runtime-install-progress";
pub const DISCOVERY_PROGRESS_EVENT: &str = "discovery-progress";

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ScanProgressLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgressEvent {
    pub scan_id: String,
    pub timestamp: String,
    pub level: ScanProgressLevel,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finding_id: Option<String>,
}

impl ScanProgressEvent {
    pub fn new(scan_id: impl Into<String>, level: ScanProgressLevel, message: impl Into<String>) -> Self {
        Self {
            scan_id: scan_id.into(),
            timestamp: OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_else(|_| "now".into()),
            level,
            message: message.into(),
            endpoint: None,
            payload: None,
            status_code: None,
            latency: None,
            finding_id: None,
        }
    }

    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    pub fn payload(mut self, payload: impl Into<String>) -> Self {
        self.payload = Some(payload.into());
        self
    }

    pub fn status_code(mut self, code: u16) -> Self {
        self.status_code = Some(code);
        self
    }

    pub fn latency(mut self, ms: u64) -> Self {
        self.latency = Some(ms);
        self
    }

    pub fn finding_id(mut self, id: impl Into<String>) -> Self {
        self.finding_id = Some(id.into());
        self
    }
}

pub fn emit_scan_progress(app: &AppHandle, event: ScanProgressEvent) {
    let _ = app.emit(SCAN_PROGRESS_EVENT, event);
}

pub fn emit_app_data_changed(app: &AppHandle, reason: &str) {
    let _ = app.emit(APP_DATA_CHANGED_EVENT, serde_json::json!({ "reason": reason }));
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInstallProgressEvent {
    pub step: String,
    pub message: String,
    pub phase: u8,
}

pub fn emit_runtime_install_progress(
    app: &AppHandle,
    step: impl Into<String>,
    message: impl Into<String>,
    phase: u8,
) {
    let _ = app.emit(
        RUNTIME_INSTALL_PROGRESS_EVENT,
        RuntimeInstallProgressEvent {
            step: step.into(),
            message: message.into(),
            phase,
        },
    );
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryProgressEvent {
    pub phase: String,
    pub processed: usize,
    pub total: usize,
    pub elapsed_ms: u64,
}

pub fn emit_discovery_progress(
    app: &AppHandle,
    phase: &str,
    processed: usize,
    total: usize,
    elapsed_ms: u64,
) {
    let _ = app.emit(
        DISCOVERY_PROGRESS_EVENT,
        DiscoveryProgressEvent {
            phase: phase.into(),
            processed,
            total,
            elapsed_ms,
        },
    );
}

/// Helper for emitting scan progress from background jobs.
#[derive(Clone)]
pub struct ScanProgressEmitter {
    app: AppHandle,
    scan_id: String,
}

impl ScanProgressEmitter {
    pub fn new(app: AppHandle, scan_id: impl Into<String>) -> Self {
        Self {
            app,
            scan_id: scan_id.into(),
        }
    }

    pub fn emit(&self, level: ScanProgressLevel, message: impl Into<String>) {
        emit_scan_progress(
            &self.app,
            ScanProgressEvent::new(self.scan_id.clone(), level, message),
        );
    }

    pub fn info(&self, message: impl Into<String>) {
        self.emit(ScanProgressLevel::Info, message);
    }

    pub fn warn(&self, message: impl Into<String>) {
        self.emit(ScanProgressLevel::Warn, message);
    }

    pub fn error(&self, message: impl Into<String>) {
        self.emit(ScanProgressLevel::Error, message);
    }

    pub fn detailed(&self, level: ScanProgressLevel, event: ScanProgressEvent) {
        emit_scan_progress(&self.app, event);
    }

    pub fn event(&self, level: ScanProgressLevel, message: impl Into<String>) -> ScanProgressEvent {
        ScanProgressEvent::new(self.scan_id.clone(), level, message)
    }
}
