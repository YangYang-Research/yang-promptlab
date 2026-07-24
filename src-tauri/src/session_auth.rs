//! Resolve browser session auth from target descriptors for attack.

use std::path::Path;

use promptlab_attack::{AttackExecutor, AttackRegistry};
use promptlab_auth::{
    auth_sessions_dir, AuthEngineConfig, AuthSessionManager, SessionAuthContext, SessionStore,
    SessionValidationStatus,
};
use promptlab_harness::{HarnessFactory, HarnessKind, TargetDescriptor};

use promptlab_storage::Database;

use crate::error::{CommandError, CommandResult};
use crate::harness_runtime::build_harness_attack_runtime_parts;
use crate::plugin_transport::PluginAwareTransport;
use crate::state::AppState;

pub async fn auth_session_manager_from_parts(
    db: Database,
    data_dir: &Path,
    auth_config: AuthEngineConfig,
) -> Result<AuthSessionManager, CommandError> {
    let vault_dir = auth_sessions_dir(data_dir);
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

pub struct AttackRuntime {
    pub transport: PluginAwareTransport,
    pub session: Option<SessionAuthContext>,
    pub harness_kind: HarnessKind,
}

impl Clone for AttackRuntime {
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
            session: self.session.clone(),
            harness_kind: self.harness_kind,
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
        state.harness_factory(),
        state.plugin_manager().clone(),
        descriptor_json,
        probe_url,
    )
    .await
}

pub async fn build_attack_runtime_parts(
    db: Database,
    data_dir: &Path,
    auth_config: AuthEngineConfig,
    harness_factory: &HarnessFactory,
    plugin_manager: std::sync::Arc<tauri::async_runtime::Mutex<promptlab_plugin_host::PluginManager>>,
    descriptor_json: &str,
    probe_url: &str,
) -> CommandResult<AttackRuntime> {
    let runtime = build_harness_attack_runtime_parts(
        db,
        data_dir,
        auth_config,
        harness_factory,
        descriptor_json,
        probe_url,
    )
    .await?;

    Ok(AttackRuntime {
        transport: PluginAwareTransport::new(runtime.transport, plugin_manager),
        session: runtime.session,
        harness_kind: runtime.descriptor.preferred_harness(),
    })
}

pub fn fallback_attack_runtime() -> AttackRuntime {
    let factory = HarnessFactory::new().expect("harness factory");
    let descriptor = TargetDescriptor {
        url: "https://localhost".into(),
        ..TargetDescriptor::default()
    };
    let harness = promptlab_attack::HarnessTransport::from_parts(
        factory,
        descriptor,
        "https://localhost".to_string(),
    );
    let plugins = std::sync::Arc::new(tauri::async_runtime::Mutex::new(
        promptlab_plugin_host::PluginManager::new(std::env::temp_dir().join("promptlab-test-plugins"))
            .expect("plugin manager"),
    ));
    AttackRuntime {
        transport: PluginAwareTransport::new(harness, plugins),
        session: None,
        harness_kind: HarnessKind::Http,
    }
}

pub fn attack_executor(transport: PluginAwareTransport) -> AttackExecutor<PluginAwareTransport> {
    AttackExecutor::new(AttackRegistry::with_builtins(), transport)
}

pub fn attack_executor_with_variants(
    transport: PluginAwareTransport,
    variants_per_test: usize,
) -> AttackExecutor<PluginAwareTransport> {
    use promptlab_attack::{MutatorConfig, MutatorKind, PayloadMutator};
    let max_per_payload = variants_per_test.saturating_sub(1);
    let mutator = PayloadMutator::new(MutatorConfig {
        enabled: MutatorKind::all().to_vec(),
        max_per_payload,
    });
    attack_executor(transport).with_mutator(mutator)
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
