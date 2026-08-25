//! Model registry IPC integration tests (Remote-only; no built-in GGUF catalog).

use std::path::Path;

use promptlab_core::{init_logging, LogOptions};
use promptlab_desktop_lib::commands::models::{models_browse_op, models_registry_info_op};
use promptlab_desktop_lib::db::open_database;
use promptlab_desktop_lib::state::AppState;

async fn make_state(dir: &Path) -> AppState {
    let db = open_database(&dir.join("promptlab.db")).await.expect("open db");
    let guard = init_logging(LogOptions::bootstrap("models-it")).unwrap();
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
async fn browse_returns_empty_without_builtin_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path()).await;

    let info = models_registry_info_op(&state).expect("registry info");
    assert_eq!(info.entry_count, 0);
    assert_eq!(info.valid_models, 0);
    assert_eq!(info.invalid_models, 0);

    let catalog = models_browse_op(&state).await.expect("browse");
    assert!(catalog.is_empty());
}
