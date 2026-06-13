//! Interactive Playwright session recording for target authentication.

use aisec_auth::{
    AuthConfig, AuthEngine, AuthMethod, AuthProfile, RecordLoginOptions, SessionStore,
};
use aisec_core::AisecError;
use serde::Serialize;
use tauri::{async_runtime::Mutex, State};

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

pub struct AuthRecordingState {
    engine: Option<AuthEngine>,
    profile_id: Option<String>,
}

impl AuthRecordingState {
    pub fn new() -> Self {
        Self {
            engine: None,
            profile_id: None,
        }
    }

    async fn ensure_engine(&mut self, state: &AppState) -> Result<(), CommandError> {
        if self.engine.is_none() {
            let vault_dir = state.data_dir().join("auth-vault");
            let config = state.auth_engine_config().clone().with_vault_dir(vault_dir);
            let store = SessionStore::new(state.database().clone(), config.vault_dir.clone())
                .await
                .map_err(CommandError::from)?;
            let engine = AuthEngine::new(config, store, None)
                .await
                .map_err(CommandError::from)?;
            self.engine = Some(engine);
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthRecordStartDto {
    pub recording: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthRecordFinishDto {
    pub session_id: String,
    pub verified: bool,
}

fn build_auth_config(
    method: &str,
    login_url: &str,
    config: Option<serde_json::Value>,
) -> Result<AuthConfig, CommandError> {
    let value = config.unwrap_or(serde_json::json!({}));

    match method {
        "username_password" => Ok(AuthConfig::UsernamePassword {
            login_url: login_url.to_string(),
            username: value
                .get("username")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            password: value
                .get("password")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            username_selector: String::new(),
            password_selector: String::new(),
            submit_selector: String::new(),
        }),
        "oauth" => Ok(AuthConfig::OAuth {
            login_url: login_url.to_string(),
            success_url_pattern: None,
            provider: None,
        }),
        _ => Err(CommandError::from(AisecError::invalid_input(format!(
            "unsupported auth recording method: {method}"
        )))),
    }
}

fn parse_auth_method(method: &str) -> Result<AuthMethod, CommandError> {
    AuthMethod::parse(method).ok_or_else(|| {
        CommandError::from(AisecError::invalid_input(format!(
            "unsupported auth recording method: {method}"
        )))
    })
}

#[tauri::command]
pub async fn auth_record_session_start(
    state: State<'_, AppState>,
    auth_state: State<'_, Mutex<AuthRecordingState>>,
    login_url: String,
    method: String,
    config: Option<serde_json::Value>,
) -> CommandResult<AuthRecordStartDto> {
    let login_url = login_url.trim();
    if login_url.is_empty() {
        return Err(CommandError::from(AisecError::invalid_input(
            "login URL is required",
        )));
    }

    let mut auth = auth_state.lock().await;
    if auth.profile_id.is_some() {
        return Err(CommandError::from(AisecError::invalid_input(
            "a browser recording is already in progress",
        )));
    }

    let auth_method = parse_auth_method(&method)?;
    let auth_config = build_auth_config(&method, login_url, config)?;

    let profile = AuthProfile {
        id: String::new(),
        project_id: None,
        name: "Target browser recording".to_string(),
        method: auth_method,
        config: auth_config,
    };

    auth.ensure_engine(&state).await?;
    let stored_profile = auth
        .engine
        .as_mut()
        .expect("auth engine initialized after ensure")
        .store()
        .create_profile(&profile)
        .await
        .map_err(CommandError::from)?;
    auth.profile_id = Some(stored_profile.id.clone());

    auth.engine
        .as_mut()
        .expect("auth engine initialized after ensure")
        .begin_interactive_recording(
            login_url,
            RecordLoginOptions {
                headed: true,
                ..RecordLoginOptions::default()
            },
        )
        .await
        .map_err(CommandError::from)?;

    Ok(AuthRecordStartDto { recording: true })
}

#[tauri::command]
pub async fn auth_record_session_finish(
    state: State<'_, AppState>,
    auth_state: State<'_, Mutex<AuthRecordingState>>,
) -> CommandResult<AuthRecordFinishDto> {
    let mut auth = auth_state.lock().await;
    let profile_id = auth.profile_id.take().ok_or_else(|| {
        CommandError::from(AisecError::invalid_input(
            "no browser recording is in progress",
        ))
    })?;

    auth.ensure_engine(&state).await?;
    let engine = auth
        .engine
        .as_mut()
        .expect("auth engine initialized after ensure");
    let (session, _recording) = engine
        .finish_interactive_recording(&profile_id)
        .await
        .map_err(CommandError::from)?;

    Ok(AuthRecordFinishDto {
        session_id: session.id,
        verified: true,
    })
}
