//! Plugin manager IPC integration tests.

use std::path::Path;

use aisec_core::{init_logging, LogOptions};
use aisec_desktop_lib::commands::plugins::{plugins_info_op, plugins_list_op, plugins_refresh_op};
use aisec_desktop_lib::db::open_database;
use aisec_desktop_lib::state::AppState;

async fn make_state(dir: &Path) -> AppState {
    let db = open_database(&dir.join("aisec.db")).await.expect("open db");
    let guard = init_logging(LogOptions::bootstrap("plugins-it")).unwrap();
    let (manager, provider, meta, harness_factory, plugin_manager) =
        aisec_desktop_lib::model_registry::open_test_model_stack(dir).expect("model stack");
    AppState::new(
        db,
        dir.to_path_buf(),
        guard,
        aisec_auth::AuthEngineConfig::default(),
        harness_factory,
        plugin_manager,
        aisec_runtime::RuntimeSupervisor::new("", dir),
        manager,
        provider,
        meta,
    )
}

#[tokio::test]
async fn refresh_discovers_sample_plugins() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path()).await;

    let plugins = plugins_refresh_op(&state).await.expect("refresh plugins");
    assert!(
        plugins.len() >= 4,
        "expected bundled sample plugins to be seeded"
    );

    let info = plugins_info_op(&state).await.expect("plugins info");
    assert!(info.discovery_count >= 1);
    assert!(info.attack_count >= 1);
    assert!(info.judge_count >= 1);

    let listed = plugins_list_op(&state).await.expect("list plugins");
    assert_eq!(listed.len(), plugins.len());
}
