use std::path::{Path, PathBuf};
use std::sync::Arc;

use promptlab_auth::AuthEngineConfig;
use promptlab_agenttrace::SharedAgentTrace;
use promptlab_core::{EnvironmentPaths, EventBus, EventLogGuard, EventRing, LogGuard};
use promptlab_harness::HarnessFactory;
use promptlab_models::LocalModelManager;
use promptlab_plugin_host::PluginManager;
use promptlab_inference::InferenceRuntimeManager;
use promptlab_runtime::RuntimeManager;
use promptlab_storage::{Database, Repositories};
use tauri::async_runtime::Mutex as AsyncMutex;

use crate::jobs::ScanJobManager;

/// Shared application state managed by Tauri and accessible from commands.
pub struct AppState {
    db: Database,
    environment: EnvironmentPaths,
    event_bus: Arc<EventBus>,
    event_ring: Arc<EventRing>,
    jobs: ScanJobManager,
    auth_engine_config: AuthEngineConfig,
    harness_factory: HarnessFactory,
    plugin_manager: Arc<AsyncMutex<PluginManager>>,
    model_manager: Arc<AsyncMutex<LocalModelManager>>,
    model_provider: promptlab_runtime::SharedModelProvider,
    runtime_manager: Arc<AsyncMutex<RuntimeManager>>,
    inference_manager: Arc<AsyncMutex<InferenceRuntimeManager>>,
    runtime_config_cache: Arc<AsyncMutex<Option<crate::commands::runtime::RuntimeConfigurationDto>>>,
    runtime_model_loading_id: Arc<AsyncMutex<Option<String>>>,
    runtime_model_testing_id: Arc<AsyncMutex<Option<String>>>,
    _log_guard: LogGuard,
    _event_log_guard: EventLogGuard,
    agent_trace: SharedAgentTrace,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: Database,
        environment: EnvironmentPaths,
        event_bus: Arc<EventBus>,
        event_ring: Arc<EventRing>,
        log_guard: LogGuard,
        event_log_guard: EventLogGuard,
        auth_engine_config: AuthEngineConfig,
        harness_factory: HarnessFactory,
        plugin_manager: Arc<AsyncMutex<PluginManager>>,
        runtime_manager: RuntimeManager,
        model_manager: Arc<AsyncMutex<LocalModelManager>>,
        model_provider: promptlab_runtime::SharedModelProvider,
        agent_trace: SharedAgentTrace,
    ) -> Self {
        let config_dir = environment.config.clone();
        Self {
            db,
            environment: environment.clone(),
            event_bus,
            event_ring,
            jobs: ScanJobManager::default(),
            auth_engine_config,
            harness_factory,
            plugin_manager,
            model_manager,
            model_provider,
            runtime_manager: Arc::new(AsyncMutex::new(runtime_manager)),
            inference_manager: Arc::new(AsyncMutex::new(InferenceRuntimeManager::new(config_dir))),
            runtime_config_cache: Arc::new(AsyncMutex::new(None)),
            runtime_model_loading_id: Arc::new(AsyncMutex::new(None)),
            runtime_model_testing_id: Arc::new(AsyncMutex::new(None)),
            _log_guard: log_guard,
            _event_log_guard: event_log_guard,
            agent_trace,
        }
    }

    pub fn database(&self) -> &Database {
        &self.db
    }

    pub fn jobs(&self) -> &ScanJobManager {
        &self.jobs
    }

    pub fn repositories(&self) -> Repositories {
        self.db.repositories()
    }

    pub fn environment(&self) -> &EnvironmentPaths {
        &self.environment
    }

    /// PromptLab root directory (`~/.promptlab`).
    pub fn root_dir(&self) -> &Path {
        &self.environment.root
    }

    /// Legacy alias — returns the PromptLab root directory.
    pub fn data_dir(&self) -> &Path {
        self.root_dir()
    }

    pub fn workspaces_dir(&self) -> &Path {
        &self.environment.workspaces
    }

    pub fn config_dir(&self) -> &Path {
        &self.environment.config
    }

    pub fn reports_dir(&self) -> PathBuf {
        self.environment.reports_dir()
    }

    pub fn models_dir(&self) -> PathBuf {
        self.environment.models.clone()
    }

    pub fn runtime_dir(&self) -> PathBuf {
        self.environment.runtime.clone()
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.environment.logs.clone()
    }

    pub fn model_manager(&self) -> &Arc<AsyncMutex<LocalModelManager>> {
        &self.model_manager
    }

    pub fn model_provider(&self) -> &promptlab_runtime::SharedModelProvider {
        &self.model_provider
    }

    pub fn runtime_manager(&self) -> &Arc<AsyncMutex<RuntimeManager>> {
        &self.runtime_manager
    }

    pub fn inference_manager(&self) -> &Arc<AsyncMutex<InferenceRuntimeManager>> {
        &self.inference_manager
    }

    pub fn runtime_config_cache(
        &self,
    ) -> &Arc<AsyncMutex<Option<crate::commands::runtime::RuntimeConfigurationDto>>> {
        &self.runtime_config_cache
    }

    pub fn runtime_model_loading_id(&self) -> &Arc<AsyncMutex<Option<String>>> {
        &self.runtime_model_loading_id
    }

    pub fn runtime_model_testing_id(&self) -> &Arc<AsyncMutex<Option<String>>> {
        &self.runtime_model_testing_id
    }

    pub fn event_bus(&self) -> &Arc<EventBus> {
        &self.event_bus
    }

    pub fn event_ring(&self) -> &Arc<EventRing> {
        &self.event_ring
    }

    pub fn auth_engine_config(&self) -> &AuthEngineConfig {
        &self.auth_engine_config
    }

    pub fn harness_factory(&self) -> &HarnessFactory {
        &self.harness_factory
    }

    pub fn plugin_manager(&self) -> &Arc<AsyncMutex<PluginManager>> {
        &self.plugin_manager
    }

    pub fn plugins_dir(&self) -> PathBuf {
        self.environment.plugins.clone()
    }

    pub fn agent_trace(&self) -> &SharedAgentTrace {
        &self.agent_trace
    }
}
