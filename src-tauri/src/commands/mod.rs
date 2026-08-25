use serde::Serialize;
use tauri::State;

use promptlab_storage::ProjectRepository;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

pub mod app;
pub mod agent_memory;
pub mod agenttrace;
pub mod attack;
pub mod attack_catalog;
pub mod domain;
pub mod environment;
pub mod generator;
pub mod models;
pub mod mutators;
pub mod planner;
pub mod plugins;
pub mod projects;
pub mod proxy;
pub mod runtime;
pub mod scan;
pub mod scan_execution;
pub mod project_summary;
pub mod scan_recommendations;
pub mod finding_recommendations;
pub mod security;
pub mod target_profile;
pub mod yazg;
pub mod wizard_scan;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoResponse {
    pub name: &'static str,
    pub version: &'static str,
    pub identifier: &'static str,
    pub platform: String,
}

/// Bootstrap health check command for IPC wiring verification.
#[tauri::command]
pub fn health() -> CommandResult<HealthResponse> {
    Ok(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Returns static application metadata.
#[tauri::command]
pub fn app_info() -> CommandResult<AppInfoResponse> {
    Ok(AppInfoResponse {
        name: "PromptLab",
        version: env!("CARGO_PKG_VERSION"),
        identifier: "com.promptlab.desktop",
        platform: detect_host_platform(),
    })
}

fn detect_host_platform() -> String {
    #[cfg(target_os = "macos")]
    {
        let version = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        return match version {
            Some(version) => format!("macOS {version}"),
            None => "macOS".into(),
        };
    }

    #[cfg(target_os = "windows")]
    {
        let raw = std::process::Command::new("cmd")
            .args(["/C", "ver"])
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .unwrap_or_default();
        if let Some(version) = parse_windows_version(&raw) {
            return version;
        }
        return "Windows".into();
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(pretty) = read_linux_pretty_name() {
            return pretty;
        }
        let version = std::fs::read_to_string("/proc/version")
            .ok()
            .and_then(|content| {
                content
                    .split_whitespace()
                    .nth(2)
                    .map(|token| token.to_string())
            });
        return match version {
            Some(version) => format!("Linux {version}"),
            None => "Linux".into(),
        };
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        format!("{} ({})", std::env::consts::OS, std::env::consts::ARCH)
    }
}

#[cfg(target_os = "windows")]
fn parse_windows_version(raw: &str) -> Option<String> {
    let start = raw.find(|c: char| c.is_ascii_digit())?;
    let end = raw[start..]
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .map(|offset| start + offset)
        .unwrap_or(raw.len());
    let version = raw[start..end].trim_matches(|c: char| c == '.' || c.is_whitespace());
    if version.is_empty() {
        return None;
    }
    let build = version
        .split('.')
        .nth(2)
        .and_then(|part| part.parse::<u32>().ok())
        .unwrap_or(0);
    let family = if build >= 22000 { "Windows 11" } else { "Windows 10" };
    Some(format!("{family} ({version})"))
}

#[cfg(target_os = "linux")]
fn read_linux_pretty_name() -> Option<String> {
    let content = std::fs::read_to_string("/etc/os-release").ok()?;
    for line in content.lines() {
        let Some(value) = line.strip_prefix("PRETTY_NAME=") else {
            continue;
        };
        let pretty = value.trim().trim_matches('"').trim();
        if !pretty.is_empty() {
            return Some(pretty.to_string());
        }
    }
    None
}

/// Database connectivity check — proves the database is reachable from a command
/// via the shared [`AppState`] repository manager.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbHealthResponse {
    pub connected: bool,
    pub path: String,
    pub size_bytes: u64,
}

#[tauri::command]
pub async fn db_health(state: State<'_, AppState>) -> CommandResult<DbHealthResponse> {
    let path = state.environment().database_path();
    let size_bytes = std::fs::metadata(&path)
        .map(|meta| meta.len())
        .unwrap_or(0);

    // Exercise the repository manager against the live pool.
    let _projects = state
        .repositories()
        .projects()
        .list()
        .await
        .map_err(CommandError::from)?;

    Ok(DbHealthResponse {
        connected: !state.database().is_closed(),
        path: path.display().to_string(),
        size_bytes,
    })
}
