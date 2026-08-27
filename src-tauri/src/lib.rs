//! PromptLab desktop application library.

pub mod agent_memory;
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
pub mod environment_persist;
pub mod plugin_interceptor;
pub mod plugin_service;
pub mod plugin_transport;
pub mod session_auth;
pub mod scan_console_log;
pub mod scan_playbook;
pub mod state;
pub mod startup;
pub mod traffic_persist;
pub mod token_usage_persist;

use promptlab_models::ModelEntry;
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
        match event {
            RunEvent::Exit => {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    tauri::async_runtime::block_on(async {
                        let reconciled =
                            commands::scan::reconcile_interrupted_scans(state.inner(), true).await;
                        if reconciled > 0 {
                            tracing::info!(
                                reconciled,
                                "marked interrupted scans as stopped on shutdown"
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
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { has_visible_windows, .. } => {
                if !has_visible_windows {
                    focus_main_window(app_handle);
                }
            }
            _ => {}
        }
    });
}

fn focus_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Build the Tauri application: initialize logging, open the database, and store
/// it in shared state. Separated from [`run`] so startup wiring is unit-testable.
fn build_app() -> Result<tauri::App, Box<dyn std::error::Error>> {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let bootstrap = promptlab_core::bootstrap_environment()
                .map_err(crate::error::CommandError::from)?;

            let _ = promptlab_core::bootstrap_proxy_settings(&bootstrap.config)
                .map_err(|err| {
                    tracing::warn!(error = %err, "failed to load proxy settings; using defaults");
                    err
                });

            let (event_bus, event_ring, event_log_guard) =
                promptlab_core::spawn_event_logger(bootstrap.logs.clone());
            let event_bus = std::sync::Arc::new(event_bus);

            let log_guard = logging::init_app_logging(&bootstrap)?;

            event_bus.info(
                promptlab_core::LogCategory::Application,
                "Application Started",
                "promptlab-desktop",
                "startup",
                "PromptLab backend starting",
            );

            let root = bootstrap.root.clone();
            let db_path = db::resolve_db_path(&bootstrap.workspaces);

            let database = match tauri::async_runtime::block_on(async {
                let database = db::open_database(&db_path).await?;
                attack_catalog::seed_attack_catalog(&database).await?;

                let vault_dir = bootstrap.auth_sessions_dir();
                let store =
                    promptlab_auth::SessionStore::new(database.clone(), vault_dir.clone())
                        .await
                        .map_err(crate::error::CommandError::from)?;
                promptlab_auth::migrate_legacy_auth_data(&database, store.secrets())
                    .await
                    .map_err(crate::error::CommandError::from)?;
                promptlab_auth::migrate_legacy_target_descriptors(&database, store.secrets())
                    .await
                    .map_err(crate::error::CommandError::from)?;
                promptlab_auth::migrate_legacy_storage_artifacts(
                    &database,
                    &root,
                    &store.encrypted_vault(),
                )
                .await
                .map_err(crate::error::CommandError::from)?;
                Ok::<_, crate::error::CommandError>(database)
            }) {
                Ok(database) => database,
                Err(err) => {
                    let message = startup::format_database_startup_error(&db_path, &err);
                    tracing::error!(
                        error = %err,
                        path = %db_path.display(),
                        "database startup failed; continuing without backend state"
                    );
                    app.manage(startup::BackendStartup::database_failed(
                        db_path.clone(),
                        message,
                    ));
                    // Keep the window alive so the frontend boot screen can show the error.
                    return Ok(());
                }
            };

            let environment = match tauri::async_runtime::block_on(
                environment_persist::hydrate_environment_paths(&database, &bootstrap),
            ) {
                Ok(paths) => paths,
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        "failed to load environment settings from database; using defaults"
                    );
                    bootstrap
                }
            };

            let vault_dir = environment.auth_sessions_dir();
            let auth_engine_config =
                promptlab_auth::AuthEngineConfig::default().with_vault_dir(vault_dir.clone());

            let model_manager = tauri::async_runtime::block_on(
                model_registry::open_model_manager_with_registry(app.handle(), &root, &database),
            )
            .map_err(crate::error::CommandError::from)?;

            let _model_list: Vec<ModelEntry> =
                model_manager.list_models().into_iter().cloned().collect();

            let (mut runtime_manager, _started) =
                tauri::async_runtime::block_on(embedded_runtime::bootstrap_runtime_manager(
                    app.handle(),
                    &root,
                    &database,
                ))
                .map_err(crate::error::CommandError::from)?;

            tauri::async_runtime::block_on(
                embedded_runtime::detect_hardware_on_startup(&mut runtime_manager),
            );

            let model_manager_arc = std::sync::Arc::new(AsyncMutex::new(model_manager));
            let model_provider: promptlab_runtime::SharedModelProvider = std::sync::Arc::new(
                promptlab_runtime::EmbeddedModelProvider::new(model_manager_arc.clone()),
            );

            let harness_factory = promptlab_harness::HarnessFactory::new()
                .map_err(crate::error::CommandError::from)?;
            let plugin_manager = std::sync::Arc::new(AsyncMutex::new(
                crate::plugin_service::bootstrap_plugin_manager(&root)
                    .map_err(crate::error::CommandError::from)?,
            ));

            let agent_trace_path = environment.root.join("agenttrace").join("agenttrace.db");
            let agent_trace = tauri::async_runtime::block_on(promptlab_agenttrace::AgentTrace::open(
                &agent_trace_path,
            ))
            .map_err(|err| crate::error::CommandError::internal(err.to_string()))?;
            let agent_trace = std::sync::Arc::new(agent_trace);

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
                agent_trace,
            ));
            app.manage(startup::BackendStartup::ok());

            let startup_state = app.state::<AppState>();
            let reconciled = tauri::async_runtime::block_on(
                commands::scan::reconcile_interrupted_scans(startup_state.inner(), false),
            );
            if reconciled > 0 {
                tracing::info!(reconciled, "marked interrupted scans as stopped on startup");
            }

            let retry_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                let state = retry_app.state::<AppState>();
                commands::scan::maybe_auto_retry_scan(state.inner(), &retry_app).await;
            });

            let app_handle = app.handle().clone();
            promptlab_inference::traffic_ensure_started();
            tauri::async_runtime::spawn(async move {
                let state = app_handle.state::<AppState>();
                {
                    let mut inference = state.inference_manager().lock().await;
                    let _ = inference.load().await;
                    let models: Vec<_> = {
                        let manager = state.model_manager().lock().await;
                        manager.list_models().into_iter().cloned().collect()
                    };
                    let before = inference.config().clone();
                    let after = crate::inference_settings::reconcile_config(before.clone(), &models);
                    if after != before {
                        *inference.config_mut() = after;
                        if let Err(err) = inference.save().await {
                            tracing::warn!(error = %err, "failed to persist runtime config migration");
                        }
                    }
                }
                commands::runtime::startup_connectivity_check(state.inner()).await;
            });

            let traffic_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                traffic_persist::bootstrap_traffic_persistence(&traffic_app).await;
            });

            let usage_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                token_usage_persist::bootstrap_token_usage_persistence(&usage_app).await;
            });

            event_bus.info(
                promptlab_core::LogCategory::Application,
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
            commands::startup_status,
            commands::app::app_clear_all_data,
            commands::environment::environment_get,
            commands::environment::environment_open_root,
            commands::environment::environment_update,
            commands::proxy::proxy_get,
            commands::proxy::proxy_set,
            commands::proxy::proxy_test_connection,
            commands::environment::logs_list_files,
            commands::environment::logs_tail,
            commands::environment::logs_recent_events,
            commands::environment::logs_emit,
            commands::environment::logs_open_folder,
            commands::activity::activity_list,
            commands::activity::activity_record,
            commands::activity::activity_replace_all,
            commands::agent_memory::agent_memory_list_sessions,
            commands::agent_memory::agent_memory_list_events,
            commands::agent_memory::agent_memory_delete_session,
            commands::agent_memory::agent_memory_list_ltm,
            commands::agenttrace::agenttrace_list_sessions,
            commands::agenttrace::agenttrace_list_traces,
            commands::agenttrace::agenttrace_get_trace,
            commands::agenttrace::agenttrace_delete_session,
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
            commands::domain::finding_import_sarif,
            commands::domain::finding_update,
            commands::domain::finding_rejudge,
            commands::domain::finding_delete,
            commands::domain::report_generate,
            commands::domain::report_list,
            commands::domain::report_list_all,
            commands::domain::report_read,
            commands::domain::report_export,
            commands::domain::report_export_scan,
            commands::scan::scan_start,
            commands::scan::scan_status,
            commands::scan::scan_pause,
            commands::scan::scan_resume,
            commands::scan::scan_stop,
            commands::scan::scan_console_tail,
            commands::wizard_scan::scan_wizard_create,
            commands::wizard_scan::scan_wizard_save,
            commands::wizard_scan::scan_wizard_load,
            commands::models::models_list,
            commands::models::models_registry_info,
            commands::models::models_registry_diagnostics,
            commands::models::models_save_third_party,
            commands::models::models_third_party_edit_form,
            commands::models::models_test_third_party,
            commands::models::models_test_connection,
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
            commands::target_profile::target_profile_verify_capability,
            commands::target_profile::target_profile_verify_ai,
            commands::target_profile::target_profile_verify_ai_classify,
            commands::target_profile::target_profile_get,
            commands::target_profile::planner_generate_from_profile,
            commands::yazg::yazg_chat,
            commands::yazg::yazg_stop,
            commands::yazg::yazg_generate_chat_title,
            commands::yazg::yazg_resolve_hilt,
            commands::yazg::yazg_chat_threads_get,
            commands::yazg::yazg_chat_threads_save,
            commands::scan_recommendations::scan_recommendations_generate,
            commands::finding_recommendations::finding_recommendations_generate,
            commands::project_summary::project_summary_generate,
            commands::planner::attack_planner_adjust,
            commands::runtime::runtime_status,
            commands::runtime::runtime_start,
            commands::runtime::runtime_stop,
            commands::runtime::runtime_delete,
            commands::runtime::runtime_restart,
            commands::runtime::runtime_health,
            commands::runtime::runtime_traffic_stats,
            commands::runtime::runtime_token_usage,
            commands::runtime::runtime_token_usage_reset,
            commands::runtime::runtime_logs,
            commands::runtime::runtime_configuration,
            commands::runtime::runtime_inference_settings,
            commands::runtime::runtime_set_inference_route,
            commands::runtime::runtime_judge_role_weights,
            commands::runtime::runtime_set_judge_role_weights,
            commands::mutators::mutator_settings_get,
            commands::mutators::mutator_settings_set,
            commands::runtime::runtime_test_connectivity,
            commands::runtime::runtime_test_inference,
            commands::security::security_audit,
            commands::security::security_migrate_secrets,
            commands::attack_catalog::attack_catalog_list,
            commands::attack_catalog::attack_catalog_categories,
            commands::attack_catalog::attack_catalog_update,
            commands::attack_catalog::attack_catalog_reset,
            commands::attack_catalog::attack_catalog_generate_prompt,
        ])
        .build(tauri::generate_context!())?;

    Ok(app)
}
