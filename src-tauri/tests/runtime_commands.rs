//! Runtime IPC command tests.

use std::path::Path;

use promptlab_core::{init_logging, LogOptions};
use promptlab_desktop_lib::commands::runtime::runtime_status_op;
use promptlab_desktop_lib::db::open_database;
use promptlab_desktop_lib::state::AppState;

async fn make_state(dir: &Path) -> AppState {
    let db = open_database(&dir.join("promptlab.db")).await.expect("open db");
    let guard = init_logging(LogOptions::bootstrap("runtime-it")).unwrap();
    let (manager, provider, harness_factory, plugin_manager) =
        promptlab_desktop_lib::model_registry::open_test_model_stack(dir, &db).await.expect("model stack");
    AppState::new(
        db,
        dir.to_path_buf(),
        guard,
        promptlab_auth::AuthEngineConfig::default(),
        harness_factory,
        plugin_manager,
        promptlab_runtime::RuntimeManager::new(dir, None),
        manager,
        provider,
    )
}

#[tokio::test]
async fn runtime_status_reports_unavailable_without_binary() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path()).await;

    let status = runtime_status_op(&state).await.expect("runtime status");
    assert_eq!(status.lifecycle_state, "not_installed");
    assert!(status.message.to_lowercase().contains("runtime"));
}
