//! Harness factory wiring for attack and discovery execution paths.

use std::path::Path;

use promptlab_attack::HarnessTransport;
use promptlab_auth::{resolve_descriptor_for_runtime, resolve_descriptor_for_wizard, SecretStore};
use promptlab_harness::{AuthMaterial, HarnessFactory, TargetDescriptor};
use tracing::warn;

use promptlab_storage::Database;

use crate::error::CommandResult;
use crate::state::AppState;

pub struct HarnessAttackRuntime {
    pub transport: HarnessTransport,
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
    _db: Database,
    _data_dir: &Path,
    _auth_config: promptlab_auth::AuthEngineConfig,
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

    let auth = AuthMaterial::default();
    let descriptor =
        promptlab_harness::adapter::descriptor_from_parts(&descriptor_json, probe_url, auth);
    let factory = base_factory.isolated();
    let transport =
        HarnessTransport::from_parts(factory.clone(), descriptor.clone(), probe_url.to_string());

    Ok(HarnessAttackRuntime {
        transport,
        descriptor,
        factory,
    })
}
