use std::path::{Path, PathBuf};

use tokio::fs;
use tracing::{info, warn};

use crate::config::RuntimeConfig;
use crate::discovery::{discover_models_in_dir, DiscoveredModel};
use crate::error::{RuntimeError, RuntimeResult};
use crate::hardware::RuntimeHardwareProfile;
use crate::local_runtime_adapter::{
    default_model_config, GfxBackend, InferRequest, InferResponse, LocalRuntimeAdapter,
    LocalRuntimeCapabilities, resolve_backend,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProcessState {
    Stopped,
    Starting,
    Running,
    Failed,
}

impl RuntimeProcessState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Failed => "failed",
        }
    }
}

/// Manages embedded libllama lifecycle — no subprocess, no HTTP.
pub struct RuntimeSupervisor {
    config: RuntimeConfig,
    adapter: LocalRuntimeAdapter,
    state: RuntimeProcessState,
    watch_enabled: bool,
    pending_model: Option<PathBuf>,
    hardware: Option<RuntimeHardwareProfile>,
}

impl RuntimeSupervisor {
    pub fn new(_app_root: impl AsRef<Path>, data_root: impl AsRef<Path>) -> Self {
        Self::with_config(RuntimeConfig::new(data_root))
    }

    pub fn with_config(config: RuntimeConfig) -> Self {
        let adapter = LocalRuntimeAdapter::new(default_model_config(None), GfxBackend::Auto);
        Self {
            config,
            adapter,
            state: RuntimeProcessState::Stopped,
            watch_enabled: true,
            pending_model: None,
            hardware: None,
        }
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    pub fn state(&self) -> RuntimeProcessState {
        self.state
    }

    pub fn runtime_available(&self) -> bool {
        self.adapter.runtime_available()
    }

    /// Legacy alias — embedded libllama is always available when compiled in.
    pub fn binary_available(&self) -> bool {
        self.runtime_available()
    }

    pub fn set_hardware_profile(&mut self, profile: RuntimeHardwareProfile) {
        self.hardware = Some(profile.clone());
        if self.adapter.is_loaded() {
            return;
        }
        let backend = resolve_backend(self.config.backend, Some(&profile));
        self.adapter
            .set_backend(gfx_from_backend(backend), Some(&profile));
    }

    pub fn set_n_gpu_layers(&mut self, n_gpu_layers: u32) {
        self.adapter.set_n_gpu_layers(n_gpu_layers);
    }

    pub fn should_watch(&self) -> bool {
        self.watch_enabled && self.state == RuntimeProcessState::Running && self.adapter.is_loaded()
    }

    pub fn set_watch_enabled(&mut self, enabled: bool) {
        self.watch_enabled = enabled;
    }

    pub fn is_process_alive(&mut self) -> bool {
        self.adapter.is_loaded() || self.adapter.is_initialized()
    }

    pub async fn is_process_alive_async(&self) -> bool {
        self.adapter.is_initialized() || self.adapter.is_loaded()
    }

    pub fn local_runtime(&self) -> &LocalRuntimeAdapter {
        &self.adapter
    }

    /// Legacy alias for callers that still reference llama_runtime.
    pub fn llama_runtime(&self) -> &LocalRuntimeAdapter {
        &self.adapter
    }

    pub async fn pid(&self) -> Option<u32> {
        None
    }

    pub fn set_pending_model(&mut self, path: impl Into<PathBuf>) {
        self.pending_model = Some(path.into());
    }

    pub async fn ensure_running(&mut self) -> RuntimeResult<()> {
        fs::create_dir_all(&self.config.models_dir)
            .await
            .map_err(|err| RuntimeError::NativeRuntimeError(err.to_string()))?;

        self.adapter.initialize().await?;

        if let Some(path) = self.pending_model.take() {
            match self.ensure_model_loaded(&path).await {
                Ok(()) => return Ok(()),
                Err(err) => {
                    warn!(
                        error = %err,
                        model = %path.display(),
                        "failed to auto-load queued model; runtime will stay idle"
                    );
                }
            }
        }

        if self.adapter.is_loaded() {
            if self.check_health().await.unwrap_or(false) {
                self.state = RuntimeProcessState::Running;
                return Ok(());
            }
            warn!("embedded libllama runtime unhealthy; reloading model");
            self.stop().await?;
        }

        let vault_models = discover_models_in_dir(&self.config.models_dir).await?;
        self.state = RuntimeProcessState::Running;
        let _ = vault_models;
        info!("runtime host ready (remote-only; no local model vault)");
        Ok(())
    }

    pub async fn ensure_model_loaded(&mut self, model_path: &Path) -> RuntimeResult<()> {
        if !self.adapter.runtime_available() {
            self.state = RuntimeProcessState::Stopped;
            return Err(RuntimeError::Unavailable);
        }

        if let Some(loaded) = self.adapter.loaded_model_path().await {
            if crate::paths::same_paths(&loaded, model_path) && self.adapter.is_loaded() {
                self.state = RuntimeProcessState::Running;
                return Ok(());
            }
        }

        if self.adapter.is_loaded() {
            self.stop().await?;
        }

        self.state = RuntimeProcessState::Starting;
        self.adapter.load_model(model_path).await.map_err(|err| {
            self.state = RuntimeProcessState::Failed;
            err
        })?;
        self.state = RuntimeProcessState::Running;
        Ok(())
    }

    pub async fn check_health(&mut self) -> RuntimeResult<bool> {
        if self.adapter.is_loaded() {
            return self.adapter.health().await;
        }
        Ok(self.adapter.is_initialized())
    }

    pub async fn list_installed_models(&self) -> RuntimeResult<Vec<DiscoveredModel>> {
        discover_models_in_dir(&self.config.models_dir).await
    }

    pub async fn stop(&mut self) -> RuntimeResult<()> {
        if let Err(err) = self.adapter.shutdown().await {
            warn!(error = %err, "failed to unload embedded libllama model");
        }
        self.state = RuntimeProcessState::Stopped;
        Ok(())
    }

    pub async fn restart(&mut self) -> RuntimeResult<()> {
        let pending = self.pending_model.clone();
        let loaded = self.adapter.loaded_model_path().await;
        self.stop().await?;
        self.ensure_running().await?;
        if let Some(path) = pending {
            self.ensure_model_loaded(&path).await?;
        } else if let Some(path) = loaded {
            self.ensure_model_loaded(&path).await?;
        }
        Ok(())
    }

    pub async fn infer(&self, request: InferRequest) -> RuntimeResult<InferResponse> {
        self.adapter.infer(request).await
    }

    pub async fn capabilities(&self) -> LocalRuntimeCapabilities {
        self.adapter.capabilities().await
    }
}

impl Drop for RuntimeSupervisor {
    fn drop(&mut self) {
        if self.adapter.is_loaded() {
            let _ = tokio::runtime::Handle::try_current().map(|handle| {
                handle.block_on(async {
                    let _ = self.adapter.shutdown().await;
                })
            });
        }
    }
}

fn gfx_from_backend(backend: crate::local_runtime_adapter::RuntimeBackend) -> GfxBackend {
    match backend {
        crate::local_runtime_adapter::RuntimeBackend::Cuda => GfxBackend::Cuda,
        crate::local_runtime_adapter::RuntimeBackend::Metal => GfxBackend::Metal,
        crate::local_runtime_adapter::RuntimeBackend::Vulkan => GfxBackend::Vulkan,
        crate::local_runtime_adapter::RuntimeBackend::Cpu => GfxBackend::Cpu,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_string_values() {
        assert_eq!(RuntimeProcessState::Running.as_str(), "running");
        assert_eq!(RuntimeProcessState::Stopped.as_str(), "stopped");
    }

    #[test]
    fn embedded_runtime_always_available() {
        let supervisor = RuntimeSupervisor::new("/tmp/promptlab", "/tmp/data");
        assert!(supervisor.runtime_available());
    }
}
