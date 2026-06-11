//! AISec desktop application library.

pub mod commands;
pub mod error;
pub mod logging;
pub mod state;

use std::sync::Mutex;

use state::AppState;
use tauri::Manager;

pub fn run() {
    let result = tauri::Builder::default()
        .setup(|app| {
            let log_guard = logging::init_app_logging(app)?;
            let state = AppState::new(log_guard);
            app.manage(Mutex::new(state));
            tracing::info!("AISec bootstrap complete");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::health, commands::app_info])
        .run(tauri::generate_context!());

    if let Err(err) = result {
        eprintln!("fatal: application exited with error: {err}");
        std::process::exit(1);
    }
}
