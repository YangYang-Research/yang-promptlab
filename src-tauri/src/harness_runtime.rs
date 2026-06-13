//! Harness factory wiring for attack and discovery execution paths.

use std::path::Path;

use aisec_attack::{AttackResult, TargetTransport, TransportRequest, TransportResponse};
use aisec_auth::{AuthSessionManager, SessionAuthContext, SessionValidationStatus};
use aisec_harness::{
    adapter::descriptor_from_parts, AuthMaterial, HarnessAttackTransport, HarnessFactory,
    PlaywrightHarness, TargetDescriptor,
};
use aisec_storage::Database;
use async_trait::async_trait;

use crate::error::CommandResult;
use crate::session_auth::{auth_session_manager_from_parts, browser_session_id};
use crate::state::AppState;

pub struct HarnessTargetTransport {
    inner: HarnessAttackTransport,
}

impl Clone for HarnessTargetTransport {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl HarnessTargetTransport {
    pub fn new(inner: HarnessAttackTransport) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl TargetTransport for HarnessTargetTransport {
    async fn send(&self, request: TransportRequest) -> AttackResult<TransportResponse> {
        let payload = request.body.clone().unwrap_or_default();
        let response = self
            .inner
            .send_payload(
                &payload,
                Some(&request.method),
                request.headers,
                None,
                request.timeout_ms,
            )
            .await
            .map_err(|err| aisec_attack::AttackError::transport(err))?;

        Ok(TransportResponse {
            status: response.status,
            headers: response.headers,
            body: response.body,
            duration_ms: response.duration_ms,
        })
    }
}

pub struct HarnessAttackRuntime {
    pub transport: HarnessTargetTransport,
    pub session: Option<SessionAuthContext>,
    pub descriptor: TargetDescriptor,
    pub factory: HarnessFactory,
}

pub async fn build_harness_attack_runtime(
    state: &AppState,
    descriptor_json: &str,
    probe_url: &str,
) -> CommandResult<HarnessAttackRuntime> {
    build_harness_attack_runtime_parts(
        state.database().clone(),
        state.data_dir(),
        state.auth_engine_config().clone(),
        descriptor_json,
        probe_url,
    )
    .await
}

pub async fn build_harness_attack_runtime_parts(
    db: Database,
    data_dir: &Path,
    auth_config: aisec_auth::AuthEngineConfig,
    descriptor_json: &str,
    probe_url: &str,
) -> CommandResult<HarnessAttackRuntime> {
    let mut auth = AuthMaterial::default();
    let mut session: Option<SessionAuthContext> = None;

    if let Some(session_id) = browser_session_id(descriptor_json) {
        let manager = auth_session_manager_from_parts(db.clone(), data_dir, auth_config.clone()).await?;
        let ctx = manager
            .validate_session(&session_id, Some(probe_url))
            .await
            .map_err(crate::error::CommandError::from)?;
        if ctx.validation_status == SessionValidationStatus::Expired {
            return Err(crate::error::CommandError::invalid_input(
                "browser session is expired; record a new login session",
            ));
        }

        let mut headers = AuthSessionManager::auth_headers(&ctx);
        if let Some(cookie) = AuthSessionManager::cookie_header_for_url(&ctx, probe_url) {
            headers.insert("Cookie".into(), cookie);
        }
        auth.headers = headers;
        auth.storage_state_path = ctx
            .storage_state_path
            .clone()
            .map(|path| path.to_string_lossy().into_owned());
        session = Some(ctx);
    }

    let descriptor = descriptor_from_parts(descriptor_json, probe_url, auth.clone());
    let mut factory = HarnessFactory::new().map_err(crate::error::CommandError::from)?;

    if descriptor.preferred_harness() == aisec_harness::HarnessKind::Playwright {
        if let Some(ctx) = &session {
            if let Some(path) = ctx.storage_state_path.clone() {
                let manager = auth_session_manager_from_parts(
                    db.clone(),
                    data_dir,
                    auth_config.clone(),
                )
                .await?;
                let playwright = PlaywrightHarness::new(
                    manager.driver().clone(),
                    Some(path),
                    auth.headers.clone(),
                    descriptor.chat_selectors.clone(),
                );
                factory = factory.with_playwright(playwright);
            }
        }
    }

    let transport = HarnessTargetTransport::new(HarnessAttackTransport::new(
        factory.clone(),
        descriptor.clone(),
        probe_url.to_string(),
    ));

    Ok(HarnessAttackRuntime {
        transport,
        session,
        descriptor,
        factory,
    })
}
