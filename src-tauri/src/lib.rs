//! AISec desktop application library.
//!
//! Backend integration layer: initializes logging, opens the SQLite database
//! (running migrations) on startup, stores the database + repository manager in
//! shared [`AppState`], and closes the pool gracefully on shutdown.

pub mod commands;
pub mod db;
pub mod dto;
pub mod error;
pub mod logging;
pub mod state;

use state::AppState;
use tauri::{Manager, RunEvent};

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

            // 3. Store Database + repository manager inside AppState for commands.
            app.manage(AppState::new(database, data_dir, log_guard));

            tracing::info!("AISec backend integration ready (database + repositories)");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health,
            commands::app_info,
            commands::db_health,
            commands::domain::project_create,
            commands::domain::project_list,
            commands::domain::project_get,
            commands::domain::project_delete,
            commands::domain::target_create,
            commands::domain::target_list,
            commands::domain::scan_create,
            commands::domain::scan_list,
            commands::domain::finding_list,
            commands::domain::report_generate,
            commands::domain::report_list,
            commands::domain::report_read,
            commands::domain::report_export,
            commands::discovery::discovery_run,
            commands::discovery::endpoint_list,
            commands::attack::attack_run_prompt_injection,
        ])
        .build(tauri::generate_context!())?;

    Ok(app)
}
