//! Backend startup status — keeps the UI alive when the database cannot open.

use serde::Serialize;
use std::path::PathBuf;

use tauri::App;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

/// Shared flag so IPC can report a soft DB/startup failure without exiting.
#[derive(Debug, Clone)]
pub struct BackendStartup {
    pub ok: bool,
    pub database_error: Option<String>,
    pub database_path: Option<PathBuf>,
}

impl BackendStartup {
    pub fn ok() -> Self {
        Self {
            ok: true,
            database_error: None,
            database_path: None,
        }
    }

    pub fn database_failed(path: PathBuf, error: impl Into<String>) -> Self {
        Self {
            ok: false,
            database_error: Some(error.into()),
            database_path: Some(path),
        }
    }

    pub fn to_dto(&self) -> BackendStartupDto {
        BackendStartupDto {
            ok: self.ok,
            database_error: self.database_error.clone(),
            database_path: self
                .database_path
                .as_ref()
                .map(|p| p.display().to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendStartupDto {
    pub ok: bool,
    pub database_error: Option<String>,
    pub database_path: Option<String>,
}

/// User-facing copy for native dialog + frontend boot screen.
pub fn format_database_startup_error(path: &std::path::Path, err: &dyn std::fmt::Display) -> String {
    format!(
        "The application database could not be opened or migrated.\n\n\
Path: {path}\n\
Error: {err}\n\n\
This usually happens after an upgrade when the existing database schema is incompatible \
with this version of PromptLab.\n\n\
Close the app, back up or delete the database file above, then open PromptLab again.",
        path = path.display(),
        err = err
    )
}

pub fn show_database_error_dialog(app: &App, message: &str) {
    app.dialog()
        .message(message)
        .title("PromptLab — Database error")
        .kind(MessageDialogKind::Error)
        .blocking_show();
}
