//! PromptLab desktop application library.

pub mod attack_catalog;
pub mod commands;
pub mod db;
pub mod dto;
pub mod error;
pub mod events;
pub mod jobs;
pub mod logging;
pub mod method_heuristic;
pub mod harness_runtime;
pub mod inference_host;
pub mod inference_settings;
pub mod model_registry;
pub mod third_party_credentials;
pub mod embedded_runtime;
pub mod plugin_service;
pub mod plugin_transport;
pub mod playwright_runtime;
pub mod runtime_watch;
pub mod session_auth;
pub mod scan_console_log;
pub mod scan_playbook;
pub mod state;

use aisec_models::ModelEntry;
use state::AppState;
use tauri::{async_runtime::Mutex as AsyncMutex, Manager, RunEvent};

pub fn run() {
    let app = match build_app() {
        Ok(app) => app,
        Err(err) => {
            eprintln!("fatal: failed to start PromptLab backend: {err}");
            std::process::exit(1);
        }
    };

    app.run(|app_handle, event| {
        if let RunEvent::Exit = event {
            if let Some(state) = app_handle.try_state::<AppState>() {
                tauri::async_runtime::block_on(async {
                    let reconciled =
                        commands::scan::reconcile_interrupted_scans(state.inner(), true).await;
                    if reconciled > 0 {
                        tracing::info!(
                            reconciled,
                            "marked interrupted scans as failed on shutdown"
                        );
                    }
                    let mut manager = state.runtime_manager().lock().await;
                    let _ = manager.stop_runtime().await;
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
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let environment = aisec_core::bootstrap_environment()
                .map_err(crate::error::CommandError::from)?;

            let (event_bus, event_ring, event_log_guard) =
                aisec_core::spawn_event_logger(environment.logs.clone());
            let event_bus = std::sync::Arc::new(event_bus);

            let log_guard = logging::init_app_logging(&environment)?;

            event_bus.info(
                aisec_core::LogCategory::Application,
                "Application Started",
                "promptlab-desktop",
                "startup",
                "PromptLab backend starting",
            );

            let root = environment.root.clone();
            let db_path = db::resolve_db_path(&environment.workspaces);

            let database = tauri::async_runtime::block_on(db::open_database(&db_path))
                .map_err(crate::error::CommandError::from)?;

            tauri::async_runtime::block_on(attack_catalog::seed_attack_catalog(&database))?;

            let vault_dir = environment.auth_sessions_dir();
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
                    &root,
                    &store.encrypted_vault(),
                )
                .await
                .map_err(crate::error::CommandError::from)?;
                Ok::<(), crate::error::CommandError>(())
            })?;

            let (mut model_manager, model_catalog_meta) = tauri::async_runtime::block_on(
                model_registry::open_model_manager_with_registry(app.handle(), &root),
            )
            .map_err(crate::error::CommandError::from)?;

            let _model_list: Vec<ModelEntry> =
                model_manager.list_models().into_iter().cloned().collect();

            let (mut runtime_manager, _started) =
                tauri::async_runtime::block_on(embedded_runtime::bootstrap_runtime_manager(
                    app.handle(),
                    &root,
                ))
                .map_err(crate::error::CommandError::from)?;

            tauri::async_runtime::block_on(
                embedded_runtime::detect_hardware_on_startup(&mut runtime_manager),
            );

            let model_manager_arc = std::sync::Arc::new(AsyncMutex::new(model_manager));
            let model_provider: aisec_runtime::SharedModelProvider = std::sync::Arc::new(
                aisec_runtime::EmbeddedModelProvider::new(model_manager_arc.clone()),
            );

            let harness_factory = aisec_harness::HarnessFactory::new()
                .map_err(crate::error::CommandError::from)?;
            let plugin_manager = std::sync::Arc::new(AsyncMutex::new(
                crate::plugin_service::bootstrap_plugin_manager(&root)
                    .map_err(crate::error::CommandError::from)?,
            ));

            app.manage(AppState::new(
                database,
                environment,
                event_bus.clone(),
                event_ring,
                log_guard,
                event_log_guard,
                auth_engine_config,
                harness_factory,
                plugin_manager,
                runtime_manager,
                model_manager_arc,
                model_provider,
                model_catalog_meta,
            ));

            let startup_state = app.state::<AppState>();
            let reconciled = tauri::async_runtime::block_on(
                commands::scan::reconcile_interrupted_scans(startup_state.inner(), false),
            );
            if reconciled > 0 {
                tracing::info!(reconciled, "marked interrupted scans as failed on startup");
            }

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = app_handle.state::<AppState>();
                {
                    let mut inference = state.inference_manager().lock().await;
                    let _ = inference.load().await;
                }
                embedded_runtime::resume_local_runtime_on_startup(&app_handle, state.inner()).await;
            });

            event_bus.info(
                aisec_core::LogCategory::Application,
                "Application Ready",
                "promptlab-desktop",
                "startup",
                "PromptLab backend integration ready",
            );
            tracing::info!("PromptLab backend integration ready");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health,
            commands::app_info,
            commands::app::app_clear_all_data,
            commands::environment::environment_get,
            commands::environment::environment_open_root,
            commands::environment::environment_update,
            commands::environment::logs_list_files,
            commands::environment::logs_tail,
            commands::environment::logs_recent_events,
            commands::environment::logs_open_folder,
            commands::db_health,
            commands::projects::project_create,
            commands::projects::project_list,
            commands::projects::project_get,
            commands::projects::project_update,
            commands::projects::project_delete,
            commands::domain::target_create,
            commands::domain::target_list,
            commands::domain::target_get,
            commands::domain::target_wizard_descriptor,
            commands::domain::target_update_descriptor,
            commands::domain::target_delete,
            commands::domain::scan_create,
            commands::domain::scan_list,
            commands::domain::scan_get,
            commands::domain::scan_delete,
            commands::domain::finding_list,
            commands::domain::finding_list_all,
            commands::domain::report_generate,
            commands::domain::report_list,
            commands::domain::report_list_all,
            commands::domain::report_read,
            commands::domain::report_export,
            commands::scan::scan_start,
            commands::scan::scan_status,
            commands::scan::scan_pause,
            commands::scan::scan_resume,
            commands::scan::scan_stop,
            commands::scan::scan_console_tail,
            commands::wizard_scan::scan_wizard_create,
            commands::wizard_scan::scan_wizard_save,
            commands::wizard_scan::scan_wizard_load,
            commands::auth::auth_record_session_start,
            commands::auth::auth_record_session_finish,
            commands::auth::auth_record_session_cancel,
            commands::auth::auth_session_validate,
            commands::auth::auth_session_status,
            commands::models::models_list,
            commands::models::models_registry_info,
            commands::models::models_registry_diagnostics,
            commands::models::models_browse,
            commands::models::models_install,
            commands::models::models_import_gguf,
            commands::models::models_save_third_party,
            commands::models::models_third_party_edit_form,
            commands::models::models_test_third_party,
            commands::models::models_test_connection,
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
            commands::target_profile::target_profile_list_templates,
            commands::target_profile::target_profile_save,
            commands::target_profile::target_profile_verify,
            commands::target_profile::target_profile_verify_connect,
            commands::target_profile::target_profile_verify_ai,
            commands::target_profile::target_profile_get,
            commands::target_profile::planner_generate_from_profile,
            commands::scan_recommendations::scan_recommendations_generate,
            commands::project_summary::project_summary_generate,
            commands::planner::attack_planner_adjust,
            commands::runtime::runtime_status,
            commands::runtime::runtime_install,
            commands::runtime::runtime_repair,
            commands::runtime::runtime_start,
            commands::runtime::runtime_stop,
            commands::runtime::runtime_delete,
            commands::runtime::runtime_load_model,
            commands::runtime::runtime_unload_model,
            commands::runtime::runtime_restart,
            commands::runtime::runtime_health,
            commands::runtime::runtime_benchmark,
            commands::runtime::runtime_logs,
            commands::runtime::runtime_hardware,
            commands::runtime::hardware_refresh,
            commands::runtime::runtime_configuration,
            commands::runtime::runtime_inference_settings,
            commands::runtime::runtime_set_inference_route,
            commands::runtime::runtime_test_connectivity,
            commands::runtime::runtime_test_inference,
            commands::security::security_audit,
            commands::security::security_migrate_secrets,
            commands::plugins::plugins_list,
            commands::plugins::plugins_refresh,
            commands::plugins::plugins_enable,
            commands::plugins::plugins_disable,
            commands::plugins::plugins_info,
            commands::attack_catalog::attack_catalog_list,
            commands::attack_catalog::attack_catalog_categories,
            commands::attack_catalog::attack_catalog_update,
            commands::attack_catalog::attack_catalog_reset,
            commands::attack_catalog::attack_catalog_generate_prompt,
        ])
        .manage(AsyncMutex::new(commands::auth::AuthRecordingState::new()))
        .build(tauri::generate_context!())?;

    Ok(app)
}
