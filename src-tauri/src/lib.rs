//! AISec desktop application library.
//!
//! Backend integration layer: initializes logging, opens the SQLite database
//! (running migrations) on startup, stores the database + repository manager in
//! shared [`AppState`], and closes the pool gracefully on shutdown.

pub mod commands;
pub mod db;
pub mod dto;
pub mod error;
pub mod events;
pub mod fingerprint_service;
pub mod jobs;
pub mod logging;
pub mod method_heuristic;
pub mod harness_runtime;
pub mod judge_config;
pub mod model_registry;
pub mod embedded_runtime;
pub mod plugin_service;
pub mod plugin_transport;
pub mod playwright_runtime;
pub mod agent_service;
pub mod planner_service;
pub mod generator_service;
pub mod runtime_watch;
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
        if let RunEvent::Exit = event {
            if let Some(state) = app_handle.try_state::<AppState>() {
                tauri::async_runtime::block_on(async {
                    let mut supervisor = state.runtime_supervisor().lock().await;
                    let _ = supervisor.stop().await;
                    tracing::info!("embedded runtime stopped (graceful shutdown)");
                    state.database().close().await;
                });
                tracing::info!("SQLite database closed (graceful shutdown)");
            }
        }
    });
}

/// Build the Tauri application: initialize logging, open the database, and store
/// it in shared state. Separated from [`run`] so startup wiring is unit-testable.
fn build_app() -> Result<tauri::App, Box<dyn std::error::Error>> {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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

            let vault_dir = aisec_auth::auth_sessions_dir(&data_dir);
            let auth_engine_config =
                playwright_runtime::resolve_auth_engine_config(app.handle())
                    .map_err(crate::error::CommandError::from)?
                    .with_vault_dir(vault_dir.clone());

            tauri::async_runtime::block_on(async {
                let store = aisec_auth::SessionStore::new(database.clone(), vault_dir.clone())
                    .await
                    .map_err(crate::error::CommandError::from)?;
                aisec_auth::migrate_legacy_auth_data(&database, store.secrets())
                    .await
                    .map_err(crate::error::CommandError::from)?;
                aisec_auth::migrate_legacy_target_descriptors(&database, store.secrets())
                    .await
                    .map_err(crate::error::CommandError::from)?;
                aisec_auth::migrate_legacy_storage_artifacts(
                    &database,
                    &data_dir,
                    &store.encrypted_vault(),
                )
                .await
                .map_err(crate::error::CommandError::from)?;
                let _ = crate::judge_config::migrate_judge_config_secrets(
                    &data_dir,
                    store.secrets(),
                )
                .await;
                Ok::<(), crate::error::CommandError>(())
            })?;

            let runtime_config =
                embedded_runtime::resolve_runtime_config(app.handle(), &data_dir);
            let llama_binary = runtime_config.binary.clone();
            let (runtime_supervisor, started) =
                tauri::async_runtime::block_on(embedded_runtime::start_embedded_runtime(
                    runtime_config,
                ))
                .map_err(crate::error::CommandError::from)?;

            let (mut model_manager, model_catalog_meta) = tauri::async_runtime::block_on(
                model_registry::open_model_manager_with_registry(app.handle(), &data_dir),
            )
            .map_err(crate::error::CommandError::from)?;
            model_manager = model_manager.with_llama_binary(llama_binary);

            let model_manager_arc = std::sync::Arc::new(AsyncMutex::new(model_manager));
            let model_provider: aisec_runtime::SharedModelProvider = std::sync::Arc::new(
                aisec_runtime::EmbeddedModelProvider::new(model_manager_arc.clone()),
            );

            let harness_factory = aisec_harness::HarnessFactory::new()
                .map_err(crate::error::CommandError::from)?;
            let plugin_manager = std::sync::Arc::new(AsyncMutex::new(
                crate::plugin_service::bootstrap_plugin_manager(&data_dir)
                    .map_err(crate::error::CommandError::from)?,
            ));

            app.manage(AppState::new(
                database,
                data_dir,
                log_guard,
                auth_engine_config,
                harness_factory,
                plugin_manager,
                runtime_supervisor,
                model_manager_arc,
                model_provider,
                model_catalog_meta,
            ));

            if started {
                runtime_watch::spawn_runtime_watch(app.handle().clone());
            }

            tracing::info!("AISec backend integration ready (database + repositories + runtime)");
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
            commands::discovery::endpoint_update,
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
            commands::models::models_registry_info,
            commands::models::models_registry_diagnostics,
            commands::models::models_browse,
            commands::models::models_install,
            commands::models::models_import_gguf,
            commands::models::models_save_third_party,
            commands::models::models_test_third_party,
            commands::models::models_import_zip,
            commands::models::models_download_start,
            commands::models::models_download_status,
            commands::models::models_download_pause,
            commands::models::models_download_resume,
            commands::models::models_download_cancel,
            commands::models::models_download_retry_verify,
            commands::models::models_download_cancel_verify,
            commands::models::models_remove,
            commands::models::models_verify,
            commands::models::models_test_inference,
            commands::models::models_test_embeddings,
            commands::models::models_vault_path,
            commands::models::models_vault_stats,
            commands::planner::planner_generate,
            commands::generator::generator_generate,
            commands::runtime::runtime_status,
            commands::runtime::runtime_restart,
            commands::runtime::runtime_stop,
            commands::security::security_audit,
            commands::security::security_migrate_secrets,
            commands::plugins::plugins_list,
            commands::plugins::plugins_refresh,
            commands::plugins::plugins_enable,
            commands::plugins::plugins_disable,
            commands::plugins::plugins_info,
        ])
        .manage(AsyncMutex::new(commands::auth::AuthRecordingState::new()))
        .build(tauri::generate_context!())?;

    Ok(app)
}
