//! Model registry IPC integration tests.

use std::path::Path;

use aisec_core::{init_logging, LogOptions};
use aisec_desktop_lib::commands::models::{models_browse_op, models_registry_info_op};
use aisec_desktop_lib::db::open_database;
use aisec_desktop_lib::state::AppState;

async fn make_state(dir: &Path) -> AppState {
    let db = open_database(&dir.join("aisec.db")).await.expect("open db");
    let guard = init_logging(LogOptions::bootstrap("models-it")).unwrap();
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
async fn browse_loads_builtin_registry() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path()).await;

    let info = models_registry_info_op(&state).expect("registry info");
    assert!(info.valid_models >= 3, "expected bundled registry entries");
    assert_eq!(info.invalid_models, 0);

    let catalog = models_browse_op(&state).await.expect("browse");
    assert!(!catalog.is_empty());
    assert!(catalog.iter().any(|e| e.id == "qwen3-8b-judge"));
    assert!(catalog.iter().any(|e| e.engine == "llama.cpp"));
    assert!(catalog.iter().any(|e| e.recommended));
}
