//! Resolve browser session auth from target descriptors for discovery and attack.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use aisec_attack::{
    AttackExecutor, AttackRegistry, HttpTransport, TargetTransport, TransportRequest,
    TransportResponse,
};
use aisec_auth::{
    AuthEngineConfig, AuthSessionManager, SessionAuthContext, SessionStore, SessionValidationStatus,
};
use aisec_discovery::SessionAuthMaterial;
use aisec_storage::Database;
use async_trait::async_trait;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

pub async fn auth_session_manager_from_parts(
    db: Database,
    data_dir: &Path,
    auth_config: AuthEngineConfig,
) -> Result<AuthSessionManager, CommandError> {
    let vault_dir = data_dir.join("auth-vault");
    let store = SessionStore::new(db, vault_dir)
        .await
        .map_err(CommandError::from)?;
    AuthSessionManager::new(store, auth_config, None)
        .await
        .map_err(CommandError::from)
}

pub async fn auth_session_manager(state: &AppState) -> Result<AuthSessionManager, CommandError> {
    auth_session_manager_from_parts(
        state.database().clone(),
        state.data_dir(),
        state.auth_engine_config().clone(),
    )
    .await
}

pub fn seed_url_from_descriptor(descriptor_json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(descriptor_json).ok()?;
    for key in ["url", "base_url", "baseUrl"] {
        if let Some(url) = value.get(key).and_then(|v| v.as_str()) {
            if !url.trim().is_empty() {
                return Some(url.trim().to_string());
            }
        }
    }
    None
}

pub fn browser_session_id(descriptor_json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(descriptor_json).ok()?;
    let auth = value.get("auth")?;
    if auth.get("engine")?.as_str()? != "playwright" {
        return None;
    }
    auth.get("session_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

pub fn uses_browser_session(descriptor_json: &str) -> bool {
    browser_session_id(descriptor_json).is_some()
}

pub fn session_auth_material(ctx: &SessionAuthContext, seed_url: &str) -> SessionAuthMaterial {
    let mut headers = AuthSessionManager::auth_headers(ctx);
    if let Some(cookie) = AuthSessionManager::cookie_header_for_url(ctx, seed_url) {
        headers.insert("Cookie".into(), cookie);
    }
    SessionAuthMaterial {
        headers,
        storage_state_path: ctx.storage_state_path.clone(),
    }
}

pub async fn validate_browser_session(
    state: &AppState,
    session_id: &str,
    probe_url: &str,
) -> CommandResult<SessionAuthContext> {
    let manager = auth_session_manager(state).await?;
    let ctx = manager
        .validate_session(session_id, Some(probe_url))
        .await
        .map_err(CommandError::from)?;
    if ctx.validation_status == SessionValidationStatus::Expired {
        return Err(CommandError::invalid_input(
            "browser session is expired; record a new login session",
        ));
    }
    Ok(ctx)
}

pub async fn resolve_discovery_auth(
    state: &AppState,
    descriptor_json: &str,
    seed_url: &str,
) -> CommandResult<Option<SessionAuthMaterial>> {
    let Some(session_id) = browser_session_id(descriptor_json) else {
        return Ok(None);
    };
    let ctx = validate_browser_session(state, &session_id, seed_url).await?;
    Ok(Some(session_auth_material(&ctx, seed_url)))
}

pub struct PlaywrightSessionTransport {
    driver: aisec_auth::SharedPlaywrightDriver,
    storage_state_path: PathBuf,
    default_headers: HashMap<String, String>,
}

impl PlaywrightSessionTransport {
    pub fn new(
        driver: aisec_auth::SharedPlaywrightDriver,
        storage_state_path: PathBuf,
        default_headers: HashMap<String, String>,
    ) -> Self {
        Self {
            driver,
            storage_state_path,
            default_headers,
        }
    }
}

impl Clone for PlaywrightSessionTransport {
    fn clone(&self) -> Self {
        Self {
            driver: self.driver.clone(),
            storage_state_path: self.storage_state_path.clone(),
            default_headers: self.default_headers.clone(),
        }
    }
}

#[async_trait]
impl TargetTransport for PlaywrightSessionTransport {
    async fn send(&self, request: TransportRequest) -> aisec_attack::AttackResult<TransportResponse> {
        let mut headers = self.default_headers.clone();
        for (key, value) in request.headers {
            headers.insert(key, value);
        }

        let result = self
            .driver
            .execute_http_request(
                &request.url,
                &request.method,
                headers,
                request.body,
                Some(self.storage_state_path.as_path()),
            )
            .await
            .map_err(|err| aisec_attack::AttackError::transport(err.client_message()))?;

        Ok(TransportResponse {
            status: result.status,
            headers: result.headers,
            body: result.body,
            duration_ms: result.duration_ms,
        })
    }
}

pub enum SessionAwareTransport {
    Http(HttpTransport),
    Browser(PlaywrightSessionTransport),
}

impl Clone for SessionAwareTransport {
    fn clone(&self) -> Self {
        match self {
            Self::Http(transport) => Self::Http(transport.clone()),
            Self::Browser(transport) => Self::Browser(transport.clone()),
        }
    }
}

#[async_trait]
impl TargetTransport for SessionAwareTransport {
    async fn send(&self, request: TransportRequest) -> aisec_attack::AttackResult<TransportResponse> {
        match self {
            Self::Http(transport) => transport.send(request).await,
            Self::Browser(transport) => transport.send(request).await,
        }
    }
}

pub struct AttackRuntime {
    pub transport: SessionAwareTransport,
    pub session: Option<SessionAuthContext>,
}

impl Clone for AttackRuntime {
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
            session: self.session.clone(),
        }
    }
}

pub async fn build_attack_runtime(
    state: &AppState,
    descriptor_json: &str,
    probe_url: &str,
) -> CommandResult<AttackRuntime> {
    build_attack_runtime_parts(
        state.database().clone(),
        state.data_dir(),
        state.auth_engine_config().clone(),
        descriptor_json,
        probe_url,
    )
    .await
}

pub async fn build_attack_runtime_parts(
    db: Database,
    data_dir: &Path,
    auth_config: AuthEngineConfig,
    descriptor_json: &str,
    probe_url: &str,
) -> CommandResult<AttackRuntime> {
    if let Some(session_id) = browser_session_id(descriptor_json) {
        let manager = auth_session_manager_from_parts(db, data_dir, auth_config).await?;
        let ctx = manager
            .validate_session(&session_id, Some(probe_url))
            .await
            .map_err(CommandError::from)?;
        if ctx.validation_status == SessionValidationStatus::Expired {
            return Err(CommandError::invalid_input(
                "browser session is expired; record a new login session",
            ));
        }

        let mut headers = AuthSessionManager::auth_headers(&ctx);
        if let Some(cookie) = AuthSessionManager::cookie_header_for_url(&ctx, probe_url) {
            headers.insert("Cookie".into(), cookie);
        }

        if let Some(path) = ctx.storage_state_path.clone() {
            let transport = SessionAwareTransport::Browser(PlaywrightSessionTransport::new(
                manager.driver().clone(),
                path,
                headers.clone(),
            ));
            return Ok(AttackRuntime {
                transport,
                session: Some(ctx),
            });
        }

        return Ok(AttackRuntime {
            transport: SessionAwareTransport::Http(
                HttpTransport::new().with_default_headers(headers),
            ),
            session: Some(ctx),
        });
    }

    Ok(AttackRuntime {
        transport: SessionAwareTransport::Http(HttpTransport::new()),
        session: None,
    })
}

pub fn attack_executor(transport: SessionAwareTransport) -> AttackExecutor<SessionAwareTransport> {
    AttackExecutor::new(AttackRegistry::with_builtins(), transport)
}

pub fn session_status_dto(ctx: &SessionAuthContext) -> AuthSessionStatusDto {
    AuthSessionStatusDto {
        session_id: ctx.session_id.clone(),
        validation_status: ctx.validation_status.as_str().to_string(),
        user_identity: ctx.user_identity.clone(),
        created_at: format_timestamp(ctx.created_at),
        expires_at: ctx.expires_at.map(format_timestamp),
        last_validated_at: ctx.last_validated_at.map(format_timestamp),
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSessionStatusDto {
    pub session_id: String,
    pub validation_status: String,
    pub user_identity: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub last_validated_at: Option<String>,
}

fn format_timestamp(value: time::OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}
