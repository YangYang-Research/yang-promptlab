//! Harness factory wiring for attack and discovery execution paths.

use std::path::Path;

use promptlab_attack::HarnessTransport;
use promptlab_auth::{resolve_descriptor_for_runtime, resolve_descriptor_for_wizard, AuthSessionManager, SecretStore, SessionAuthContext, SessionValidationStatus};
use promptlab_harness::{AuthMaterial, HarnessFactory, PlaywrightHarness, TargetDescriptor};
use tracing::warn;

use promptlab_storage::Database;

use crate::error::CommandResult;
use crate::session_auth::{auth_session_manager_from_parts, browser_session_id};
use crate::state::AppState;

pub struct HarnessAttackRuntime {
    pub transport: HarnessTransport,
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
        state.harness_factory(),
        descriptor_json,
        probe_url,
    )
    .await
}

pub async fn build_harness_attack_runtime_parts(
    db: Database,
    data_dir: &Path,
    auth_config: promptlab_auth::AuthEngineConfig,
    base_factory: &HarnessFactory,
    descriptor_json: &str,
    probe_url: &str,
) -> CommandResult<HarnessAttackRuntime> {
    let secrets = SecretStore::new().map_err(crate::error::CommandError::from)?;
    let descriptor_json = match resolve_descriptor_for_runtime(descriptor_json, &secrets) {
        Ok(resolved) => resolved,
        Err(err) => {
            warn!(
                error = %err,
                "descriptor strict resolve failed for harness runtime; using wizard fallback"
            );
            resolve_descriptor_for_wizard(descriptor_json, &secrets)
                .unwrap_or_else(|_| descriptor_json.to_string())
        }
    };

    let mut auth = AuthMaterial::default();
    let mut session: Option<SessionAuthContext> = None;

    if let Some(session_id) = browser_session_id(&descriptor_json) {
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

    let descriptor = promptlab_harness::adapter::descriptor_from_parts(&descriptor_json, probe_url, auth.clone());
    let mut factory = base_factory.clone();

    if descriptor.preferred_harness() == promptlab_harness::HarnessKind::Playwright {
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

    let transport = HarnessTransport::from_parts(factory.clone(), descriptor.clone(), probe_url.to_string());

    Ok(HarnessAttackRuntime {
        transport,
        session,
        descriptor,
        factory,
    })
}
