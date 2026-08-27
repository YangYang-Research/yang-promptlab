//! Attack runtime helpers (credential auth via descriptor + keychain).

use std::path::Path;

use promptlab_attack::{AttackExecutor, AttackRegistry};
use promptlab_auth::AuthEngineConfig;
use promptlab_harness::{HarnessFactory, HarnessKind, TargetDescriptor};

use promptlab_storage::Database;

use crate::error::CommandResult;
use crate::harness_runtime::build_harness_attack_runtime_parts;
use crate::plugin_interceptor::PluginHarnessInterceptor;
use crate::plugin_transport::PluginAwareTransport;
use crate::state::AppState;

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

pub struct AttackRuntime {
    pub transport: PluginAwareTransport,
    pub harness_kind: HarnessKind,
}

impl Clone for AttackRuntime {
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
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

    let _ = runtime.factory.add_interceptor(std::sync::Arc::new(
        PluginHarnessInterceptor::new(plugin_manager.clone()),
    ));

    Ok(AttackRuntime {
        transport: PluginAwareTransport::new(runtime.transport, plugin_manager),
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
        promptlab_plugin_host::PluginManager::new(
            std::env::temp_dir().join("promptlab-test-plugins"),
        )
        .expect("plugin manager"),
    ));
    AttackRuntime {
        transport: PluginAwareTransport::new(harness, plugins),
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
    let mutator = if max_per_payload == 0 {
        PayloadMutator::identity()
    } else {
        PayloadMutator::new(MutatorConfig {
            enabled: MutatorKind::all().to_vec(),
            max_per_payload,
        })
    };
    attack_executor(transport).with_mutator(mutator)
}
