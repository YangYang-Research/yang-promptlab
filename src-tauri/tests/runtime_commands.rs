//! Runtime IPC command tests.

use std::path::Path;

use aisec_core::{init_logging, LogOptions};
use aisec_desktop_lib::commands::runtime::runtime_status_op;
use aisec_desktop_lib::db::open_database;
use aisec_desktop_lib::state::AppState;

async fn make_state(dir: &Path) -> AppState {
    let db = open_database(&dir.join("aisec.db")).await.expect("open db");
    let guard = init_logging(LogOptions::bootstrap("runtime-it")).unwrap();
    let (manager, provider, meta) =
        aisec_desktop_lib::model_registry::open_test_model_stack(dir).expect("model stack");
    AppState::new(
        db,
        dir.to_path_buf(),
        guard,
        aisec_auth::AuthEngineConfig::default(),
        aisec_runtime::RuntimeSupervisor::new("", dir),
        manager,
        provider,
        meta,
    )
}

#[tokio::test]
async fn runtime_status_reports_unavailable_without_binary() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path()).await;

    let status = runtime_status_op(&state).await.expect("runtime status");
    assert_eq!(status.state, "stopped");
    assert!(!status.binary_available || !status.healthy);
    assert!(status.message.contains("runtime") || status.message.contains("stopped"));
}
