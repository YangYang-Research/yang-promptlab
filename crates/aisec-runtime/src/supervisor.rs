use std::path::{Path, PathBuf};

use tokio::fs;
use tracing::{info, warn};

use crate::config::RuntimeConfig;
use crate::discovery::{check_health, discover_models_in_dir, DiscoveredModel};
use crate::error::{RuntimeError, RuntimeResult};
use crate::runtime::{LlamaCppRuntime, LlamaCppRuntimeConfig};

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

pub struct RuntimeSupervisor {
    config: RuntimeConfig,
    runtime: LlamaCppRuntime,
    state: RuntimeProcessState,
    watch_enabled: bool,
    pending_model: Option<PathBuf>,
}

impl RuntimeSupervisor {
    pub fn new(app_root: impl AsRef<Path>, data_root: impl AsRef<Path>) -> Self {
        Self::with_config(RuntimeConfig::new(app_root, data_root))
    }

    pub fn with_config(config: RuntimeConfig) -> Self {
        let runtime_config = LlamaCppRuntimeConfig {
            binary_path: config.binary.clone(),
            host: config.host.clone(),
            port: config.port,
            n_gpu_layers: 0,
            ctx_size: 4096,
            startup_timeout_ms: 30_000,
        };
        Self {
            config,
            runtime: LlamaCppRuntime::new(runtime_config),
            state: RuntimeProcessState::Stopped,
            watch_enabled: true,
            pending_model: None,
        }
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    pub fn state(&self) -> RuntimeProcessState {
        self.state
    }

    pub fn binary_path(&self) -> &Path {
        &self.config.binary
    }

    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    pub fn binary_available(&self) -> bool {
        self.config.binary_available()
    }

    pub fn should_watch(&self) -> bool {
        self.watch_enabled && self.state == RuntimeProcessState::Running && self.runtime.is_loaded()
    }

    pub fn set_watch_enabled(&mut self, enabled: bool) {
        self.watch_enabled = enabled;
    }

    pub fn is_process_alive(&mut self) -> bool {
        if !self.runtime.is_loaded() {
            return self.binary_available();
        }
        // When a model is loaded, rely on health rather than subprocess wait handles.
        true
    }

    pub fn llama_runtime(&self) -> &LlamaCppRuntime {
        &self.runtime
    }

    /// Queue a GGUF model to load on the next `ensure_running` / `ensure_model_loaded`.
    pub fn set_pending_model(&mut self, path: impl Into<PathBuf>) {
        self.pending_model = Some(path.into());
    }

    /// Start the embedded llama.cpp runtime when the binary is present.
    ///
    /// When no model is queued, the supervisor enters a **ready-idle** state if the
    /// binary exists and the vault contains GGUF files. When a model path is pending
    /// or passed, `llama-server` is spawned for that GGUF file.
    pub async fn ensure_running(&mut self) -> RuntimeResult<()> {
        if !self.config.binary_available() {
            self.state = RuntimeProcessState::Stopped;
            return Err(RuntimeError::Unavailable);
        }

        fs::create_dir_all(&self.config.models_dir)
            .await
            .map_err(|err| RuntimeError::Process(err.to_string()))?;

        if let Some(path) = self.pending_model.take() {
            return self.ensure_model_loaded(&path).await;
        }

        if self.runtime.is_loaded() {
            if self.check_health().await.unwrap_or(false) {
                self.state = RuntimeProcessState::Running;
                return Ok(());
            }
            warn!("embedded llama.cpp runtime unhealthy; reloading");
            self.stop().await?;
        }

        let vault_models = discover_models_in_dir(&self.config.models_dir).await?;
        if vault_models.is_empty() {
            self.state = RuntimeProcessState::Running;
            info!(
                binary = %self.config.binary.display(),
                base_url = %self.config.base_url,
                "embedded llama.cpp runtime ready (idle; no GGUF in vault)"
            );
            return Ok(());
        }

        self.state = RuntimeProcessState::Running;
        info!(
            binary = %self.config.binary.display(),
            base_url = %self.config.base_url,
            gguf_count = vault_models.len(),
            "embedded llama.cpp runtime ready (idle; load model on demand)"
        );
        Ok(())
    }

    /// Load a GGUF model into the supervised llama.cpp server.
    pub async fn ensure_model_loaded(&mut self, model_path: &Path) -> RuntimeResult<()> {
        if !self.config.binary_available() {
            self.state = RuntimeProcessState::Stopped;
            return Err(RuntimeError::Unavailable);
        }

        if self.runtime.is_loaded() {
            if let Some(loaded) = self.runtime.loaded_model_path().await {
                if loaded == model_path && self.check_health().await.unwrap_or(false) {
                    self.state = RuntimeProcessState::Running;
                    return Ok(());
                }
            }
            self.stop().await?;
        }

        self.state = RuntimeProcessState::Starting;
        self.runtime.load_model(model_path).await.map_err(|err| {
            self.state = RuntimeProcessState::Failed;
            err
        })?;
        self.state = RuntimeProcessState::Running;
        Ok(())
    }

    pub async fn wait_for_health(&self) -> RuntimeResult<bool> {
        for attempt in 0..20 {
            if check_health(
                Some(&self.config.base_url),
                Some(&self.config.models_dir),
            )
            .await?
            {
                return Ok(true);
            }
            tokio::time::sleep(std::time::Duration::from_millis(250 * (attempt + 1) as u64))
                .await;
        }
        Ok(false)
    }

    pub async fn check_health(&mut self) -> RuntimeResult<bool> {
        if self.runtime.is_loaded() {
            return self.runtime.health().await;
        }
        check_health(
            Some(&self.config.base_url),
            Some(&self.config.models_dir),
        )
        .await
    }

    pub async fn list_installed_models(&self) -> RuntimeResult<Vec<DiscoveredModel>> {
        discover_models_in_dir(&self.config.models_dir).await
    }

    pub async fn stop(&mut self) -> RuntimeResult<()> {
        if let Err(err) = self.runtime.shutdown().await {
            warn!(error = %err, "failed to stop embedded llama.cpp runtime");
        }
        self.state = RuntimeProcessState::Stopped;
        Ok(())
    }

    pub async fn restart(&mut self) -> RuntimeResult<()> {
        let pending = self.pending_model.clone();
        let loaded = self.runtime.loaded_model_path().await;
        self.stop().await?;
        self.ensure_running().await?;
        if let Some(path) = pending {
            self.ensure_model_loaded(&path).await?;
        } else if let Some(path) = loaded {
            self.ensure_model_loaded(&path).await?;
        }
        Ok(())
    }
}

impl Drop for RuntimeSupervisor {
    fn drop(&mut self) {
        if self.runtime.is_loaded() {
            let _ = tokio::runtime::Handle::try_current().map(|handle| {
                handle.block_on(async {
                    let _ = self.runtime.shutdown().await;
                })
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::bundled_llama_server_binary;

    #[test]
    fn resolves_binary_path() {
        let supervisor = RuntimeSupervisor::new("/tmp/aisec", "/tmp/data");
        assert!(supervisor
            .binary_path()
            .ends_with(if cfg!(windows) {
                "llama-server.exe"
            } else {
                "llama-server"
            }));
        assert_eq!(
            supervisor.binary_path(),
            bundled_llama_server_binary("/tmp/aisec")
        );
    }

    #[test]
    fn state_string_values() {
        assert_eq!(RuntimeProcessState::Running.as_str(), "running");
        assert_eq!(RuntimeProcessState::Stopped.as_str(), "stopped");
    }
}
