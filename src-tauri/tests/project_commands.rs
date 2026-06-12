//! Integration tests for the Project IPC command layer.
//!
//! Exercises the exact logic the `#[tauri::command]` wrappers run (`*_op`
//! functions) against a real file-backed SQLite database — no mocks. Proves
//! create / list / get / delete behavior and, crucially, that a created project
//! **persists in SQLite** (survives reopening the database).

use std::path::Path;

use aisec_core::{init_logging, LogOptions};
use aisec_desktop_lib::commands::domain::{
    project_create_op, project_delete_op, project_get_op, project_list_op,
};
use aisec_desktop_lib::db::open_database;
use aisec_desktop_lib::state::AppState;

/// Build an `AppState` backed by a real SQLite file under `dir`.
async fn make_state(dir: &Path) -> AppState {
    let db = open_database(&dir.join("aisec.db")).await.expect("open db");
    let guard = init_logging(LogOptions::bootstrap("project-it")).unwrap();
    AppState::new(db, dir.to_path_buf(), guard)
}

#[tokio::test]
async fn project_create_persists_and_get_returns_it() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path()).await;

    let created = project_create_op(&state, "Acme Pentest".into(), Some("Q2 review".into()))
        .await
        .expect("create");
    assert_eq!(created.name, "Acme Pentest");
    assert_eq!(created.description.as_deref(), Some("Q2 review"));
    assert!(!created.id.is_empty());

    let got = project_get_op(&state, created.id.clone()).await.expect("get");
    assert_eq!(got.id, created.id);
    assert_eq!(got.name, "Acme Pentest");
}

#[tokio::test]
async fn project_list_returns_all_created() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path()).await;

    assert!(project_list_op(&state).await.unwrap().is_empty());

    project_create_op(&state, "Alpha".into(), None).await.unwrap();
    project_create_op(&state, "Beta".into(), None).await.unwrap();

    let all = project_list_op(&state).await.expect("list");
    assert_eq!(all.len(), 2);
    let names: Vec<&str> = all.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"Alpha"));
    assert!(names.contains(&"Beta"));
}

#[tokio::test]
async fn project_get_missing_returns_typed_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path()).await;

    let err = project_get_op(&state, "does-not-exist".into())
        .await
        .expect_err("should error");
    assert_eq!(err.code, "NOT_FOUND");
}

#[tokio::test]
async fn project_delete_removes_from_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(dir.path()).await;

    let project = project_create_op(&state, "Temporary".into(), None).await.unwrap();
    project_delete_op(&state, project.id.clone()).await.expect("delete");

    assert!(project_get_op(&state, project.id.clone()).await.is_err());
    assert!(project_list_op(&state).await.unwrap().is_empty());

    // Deleting again is a typed not-found error (no silent success).
    let err = project_delete_op(&state, project.id).await.expect_err("re-delete errors");
    assert_eq!(err.code, "NOT_FOUND");
}

/// Acceptance criterion: a created project persists in SQLite — it is still
/// present after the database connection is dropped and the file is reopened.
#[tokio::test]
async fn project_persists_across_database_reopen() {
    let dir = tempfile::tempdir().unwrap();

    let project_id = {
        let state = make_state(dir.path()).await;
        let created = project_create_op(&state, "Persisted".into(), Some("on disk".into()))
            .await
            .expect("create");
        created.id
        // `state` (and its SQLite connection pool) is dropped here.
    };

    // Reopen the SAME on-disk database in a fresh AppState.
    let reopened = make_state(dir.path()).await;

    let listed = project_list_op(&reopened).await.expect("list after reopen");
    assert_eq!(listed.len(), 1, "project must survive reopen");
    assert_eq!(listed[0].id, project_id);

    let got = project_get_op(&reopened, project_id).await.expect("get after reopen");
    assert_eq!(got.name, "Persisted");
    assert_eq!(got.description.as_deref(), Some("on disk"));
}
