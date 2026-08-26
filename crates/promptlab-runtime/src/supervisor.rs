use std::path::Path;

use tokio::fs;
use tracing::info;

use crate::config::RuntimeConfig;
use crate::error::{RuntimeError, RuntimeResult};
use crate::hardware::RuntimeHardwareProfile;

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

/// Thin runtime host — remote providers / Ollama HTTP only.
pub struct RuntimeSupervisor {
    config: RuntimeConfig,
    state: RuntimeProcessState,
    hardware: Option<RuntimeHardwareProfile>,
}

impl RuntimeSupervisor {
    pub fn new(_app_root: impl AsRef<Path>, data_root: impl AsRef<Path>) -> Self {
        Self::with_config(RuntimeConfig::new(data_root))
    }

    pub fn with_config(config: RuntimeConfig) -> Self {
        Self {
            config,
            state: RuntimeProcessState::Stopped,
            hardware: None,
        }
    }

    pub fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    pub fn state(&self) -> RuntimeProcessState {
        self.state
    }

    pub fn set_hardware_profile(&mut self, profile: RuntimeHardwareProfile) {
        self.hardware = Some(profile);
    }

    pub fn hardware(&self) -> Option<&RuntimeHardwareProfile> {
        self.hardware.as_ref()
    }

    pub fn is_process_alive(&mut self) -> bool {
        self.state == RuntimeProcessState::Running
    }

    pub async fn is_process_alive_async(&self) -> bool {
        self.state == RuntimeProcessState::Running
    }

    pub async fn pid(&self) -> Option<u32> {
        None
    }

    pub async fn ensure_running(&mut self) -> RuntimeResult<()> {
        fs::create_dir_all(&self.config.models_dir)
            .await
            .map_err(|err| RuntimeError::NativeRuntimeError(err.to_string()))?;
        self.state = RuntimeProcessState::Running;
        info!("runtime host ready (remote providers / Ollama HTTP)");
        Ok(())
    }

    /// Host ready when Running.
    pub async fn check_health(&mut self) -> RuntimeResult<bool> {
        Ok(self.state == RuntimeProcessState::Running)
    }

    pub async fn stop(&mut self) -> RuntimeResult<()> {
        self.state = RuntimeProcessState::Stopped;
        Ok(())
    }

    pub async fn restart(&mut self) -> RuntimeResult<()> {
        self.stop().await?;
        self.ensure_running().await
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

    #[tokio::test]
    async fn ensure_running_marks_host_ready() {
        let dir = tempfile::tempdir().unwrap();
        let mut supervisor = RuntimeSupervisor::new("/tmp/promptlab", dir.path());
        supervisor.ensure_running().await.unwrap();
        assert_eq!(supervisor.state(), RuntimeProcessState::Running);
        assert!(supervisor.check_health().await.unwrap());
    }
}
