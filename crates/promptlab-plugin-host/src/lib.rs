//! PromptLab Plugin Host — plugin manager, sandbox, permissions, lifecycle.

pub mod error;
pub mod lifecycle;
pub mod manifest;
pub mod manager;
pub mod integrations;
pub mod persistence;
pub mod permissions;
pub mod sandbox;
pub mod types;

pub use error::{PluginError, PluginResult};
pub use lifecycle::PluginLifecycle;
pub use manifest::{PluginManifest, MANIFEST_FILE, HOST_API_VERSION};
pub use manager::PluginManager;
pub use integrations::{
    collect_discovery_endpoints, evaluate_with_judge_plugins, invoke_enabled_by_type,
    mutate_attack_payload, PluginDiscoveryEndpoint, PluginJudgeSignal,
};
pub use persistence::{load_enabled_ids, persist_enabled, restore_enabled, state_file_path};
pub use permissions::PermissionGuard;
pub use sandbox::SandboxRunner;
pub use types::*;

/// Default plugin manager using `./plugins` directory.
pub fn default_manager() -> PluginResult<PluginManager> {
    let dir = std::env::var("PROMPTLAB_PLUGINS_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("./plugins"));
    PluginManager::new(dir)
}
