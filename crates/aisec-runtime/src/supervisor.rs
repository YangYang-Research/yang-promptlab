use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::fs;
use tokio::process::{Child, Command};
use tokio::time::sleep;
use tracing::{info, warn};

use crate::config::RuntimeConfig;
use crate::discovery::{check_health, discover_models, DiscoveredModel};
use crate::error::{RuntimeError, RuntimeResult};

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
    child: Option<Child>,
    state: RuntimeProcessState,
    watch_enabled: bool,
}

impl RuntimeSupervisor {
    pub fn new(app_root: impl AsRef<Path>, data_root: impl AsRef<Path>) -> Self {
        Self::with_config(RuntimeConfig::new(app_root, data_root))
    }

    pub fn with_config(config: RuntimeConfig) -> Self {
        Self {
            config,
            child: None,
            state: RuntimeProcessState::Stopped,
            watch_enabled: true,
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
        self.watch_enabled && self.state == RuntimeProcessState::Running
    }

    pub fn set_watch_enabled(&mut self, enabled: bool) {
        self.watch_enabled = enabled;
    }

    pub fn is_process_alive(&mut self) -> bool {
        self.child.as_mut().is_some_and(|child| {
            child
                .try_wait()
                .ok()
                .flatten()
                .is_none()
        })
    }

    /// Start the embedded runtime if a binary is present; no-op when unavailable.
    pub async fn ensure_running(&mut self) -> RuntimeResult<()> {
        if self.state == RuntimeProcessState::Running && self.is_process_alive() {
            if self.check_health().await.unwrap_or(false) {
                return Ok(());
            }
            warn!("embedded runtime running but unhealthy; restarting");
            self.stop().await?;
        }

        if !self.config.binary_available() {
            self.state = RuntimeProcessState::Stopped;
            return Err(RuntimeError::Unavailable);
        }

        fs::create_dir_all(&self.config.models_dir)
            .await
            .map_err(|err| RuntimeError::Process(err.to_string()))?;

        self.state = RuntimeProcessState::Starting;

        let mut command = Command::new(&self.config.binary);
        command
            .arg("serve")
            .env("OLLAMA_MODELS", &self.config.models_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        if let Ok(host) = std::env::var("OLLAMA_HOST") {
            if !host.trim().is_empty() {
                command.env("OLLAMA_HOST", host);
            }
        }

        let mut child = command
            .spawn()
            .map_err(|err| RuntimeError::Process(err.to_string()))?;

        sleep(Duration::from_millis(750)).await;

        match child.try_wait() {
            Ok(Some(status)) => {
                self.state = RuntimeProcessState::Failed;
                Err(RuntimeError::Process(format!(
                    "embedded runtime exited early with {status}"
                )))
            }
            Ok(None) => {
                self.child = Some(child);
                if self.wait_for_health().await? {
                    self.state = RuntimeProcessState::Running;
                    info!(
                        path = %self.config.binary.display(),
                        base_url = %self.config.base_url,
                        models_dir = %self.config.models_dir.display(),
                        "embedded runtime started"
                    );
                    Ok(())
                } else {
                    self.state = RuntimeProcessState::Failed;
                    Err(RuntimeError::Process(
                        "embedded runtime started but failed health check".into(),
                    ))
                }
            }
            Err(err) => {
                self.state = RuntimeProcessState::Failed;
                Err(RuntimeError::Process(err.to_string()))
            }
        }
    }

    pub async fn wait_for_health(&self) -> RuntimeResult<bool> {
        for attempt in 0..20 {
            if check_health(Some(&self.config.base_url)).await? {
                return Ok(true);
            }
            sleep(Duration::from_millis(250 * (attempt + 1) as u64)).await;
        }
        Ok(false)
    }

    pub async fn check_health(&mut self) -> RuntimeResult<bool> {
        if !self.is_process_alive() {
            return Ok(false);
        }
        check_health(Some(&self.config.base_url)).await
    }

    pub async fn list_installed_models(&self) -> RuntimeResult<Vec<DiscoveredModel>> {
        discover_models(Some(&self.config.base_url)).await
    }

    pub async fn stop(&mut self) -> RuntimeResult<()> {
        if let Some(mut child) = self.child.take() {
            if let Err(err) = child.kill().await {
                warn!(error = %err, "failed to stop embedded runtime");
            }
        }
        self.state = RuntimeProcessState::Stopped;
        Ok(())
    }

    pub async fn restart(&mut self) -> RuntimeResult<()> {
        self.stop().await?;
        self.ensure_running().await
    }
}

impl Drop for RuntimeSupervisor {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::bundled_ollama_binary;

    #[test]
    fn resolves_binary_path() {
        let supervisor = RuntimeSupervisor::new("/tmp/aisec", "/tmp/data");
        assert!(supervisor
            .binary_path()
            .ends_with(if cfg!(windows) { "ollama.exe" } else { "ollama" }));
        assert_eq!(
            supervisor.binary_path(),
            bundled_ollama_binary("/tmp/aisec")
        );
    }

    #[test]
    fn state_string_values() {
        assert_eq!(RuntimeProcessState::Running.as_str(), "running");
        assert_eq!(RuntimeProcessState::Stopped.as_str(), "stopped");
    }
}
