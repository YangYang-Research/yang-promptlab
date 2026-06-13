//! AISec desktop application library.
//!
//! Backend integration layer: initializes logging, opens the SQLite database
//! (running migrations) on startup, stores the database + repository manager in
//! shared [`AppState`], and closes the pool gracefully on shutdown.

pub mod commands;
pub mod db;
pub mod dto;
pub mod error;
pub mod fingerprint_service;
pub mod jobs;
pub mod logging;
pub mod judge_config;
pub mod playwright_runtime;
pub mod session_auth;
pub mod state;

use state::AppState;
use tauri::{async_runtime::Mutex as AsyncMutex, Manager, RunEvent};

pub fn run() {
    let app = match build_app() {
        Ok(app) => app,
        Err(err) => {
            eprintln!("fatal: failed to start AISec backend: {err}");
            std::process::exit(1);
        }
    };

    app.run(|app_handle, event| {
        // 4. Graceful shutdown: flush and close the SQLite pool on exit.
        if let RunEvent::Exit = event {
            if let Some(state) = app_handle.try_state::<AppState>() {
                tauri::async_runtime::block_on(state.database().close());
                tracing::info!("SQLite database closed (graceful shutdown)");
            }
        }
    });
}

/// Build the Tauri application: initialize logging, open the database, and store
/// it in shared state. Separated from [`run`] so startup wiring is unit-testable.
fn build_app() -> Result<tauri::App, Box<dyn std::error::Error>> {
    let app = tauri::Builder::default()
        .setup(|app| {
            // 5. Logging.
            let log_guard = logging::init_app_logging(app)?;

            // 1-2. Resolve the database path and open SQLite (migrations applied).
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|err| aisec_core::AisecError::config(err.to_string()))
                .map_err(crate::error::CommandError::from)?;
            let db_path = db::resolve_db_path(&data_dir);

            let database = tauri::async_runtime::block_on(db::open_database(&db_path))
                .map_err(crate::error::CommandError::from)?;

            let auth_engine_config =
                playwright_runtime::resolve_auth_engine_config(app.handle())
                    .map_err(crate::error::CommandError::from)?;

            // 3. Store Database + repository manager inside AppState for commands.
            app.manage(AppState::new(database, data_dir, log_guard, auth_engine_config));

            tracing::info!("AISec backend integration ready (database + repositories)");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health,
            commands::app_info,
            commands::db_health,
            commands::projects::project_create,
            commands::projects::project_list,
            commands::projects::project_get,
            commands::projects::project_update,
            commands::projects::project_delete,
            commands::domain::target_create,
            commands::domain::target_list,
            commands::domain::target_get,
            commands::domain::scan_create,
            commands::domain::scan_list,
            commands::domain::scan_get,
            commands::domain::finding_list,
            commands::domain::finding_list_all,
            commands::domain::report_generate,
            commands::domain::report_list,
            commands::domain::report_list_all,
            commands::domain::report_read,
            commands::domain::report_export,
            commands::discovery::discovery_run,
            commands::discovery::endpoint_list,
            commands::discovery::endpoint_create,
            commands::attack::attack_run_prompt_injection,
            commands::scan::scan_start,
            commands::scan::scan_status,
            commands::scan::scan_pause,
            commands::scan::scan_resume,
            commands::scan::scan_stop,
            commands::auth::auth_record_session_start,
            commands::auth::auth_record_session_finish,
            commands::auth::auth_record_session_cancel,
            commands::auth::auth_session_validate,
            commands::auth::auth_session_status,
            commands::judge::judge_config_get,
            commands::judge::judge_config_save,
            commands::judge::judge_test_connectivity,
            commands::judge::judge_test_model,
            commands::models::models_list,
            commands::models::models_browse,
            commands::models::models_install,
            commands::models::models_remove,
            commands::models::models_verify,
            commands::models::models_test_inference,
            commands::models::models_test_embeddings,
            commands::models::models_vault_path,
        ])
        .manage(AsyncMutex::new(commands::auth::AuthRecordingState::new()))
        .build(tauri::generate_context!())?;

    Ok(app)
}
