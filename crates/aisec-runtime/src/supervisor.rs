use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};
use tokio::time::sleep;
use tracing::{info, warn};

use crate::error::{RuntimeError, RuntimeResult};
use crate::paths::bundled_ollama_binary;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProcessState {
    Stopped,
    Starting,
    Running,
    Failed,
}

pub struct RuntimeSupervisor {
    binary: PathBuf,
    child: Option<Child>,
    state: RuntimeProcessState,
}

impl RuntimeSupervisor {
    pub fn new(app_root: impl AsRef<Path>) -> Self {
        Self {
            binary: bundled_ollama_binary(app_root),
            child: None,
            state: RuntimeProcessState::Stopped,
        }
    }

    pub fn state(&self) -> RuntimeProcessState {
        self.state
    }

    pub fn binary_path(&self) -> &Path {
        &self.binary
    }

    pub async fn ensure_running(&mut self) -> RuntimeResult<()> {
        if self.state == RuntimeProcessState::Running {
            if self.child.as_mut().is_some_and(|child| {
                child
                    .try_wait()
                    .ok()
                    .flatten()
                    .is_none()
            }) {
                return Ok(());
            }
        }

        if !self.binary.exists() {
            self.state = RuntimeProcessState::Stopped;
            return Err(RuntimeError::Unavailable);
        }

        self.state = RuntimeProcessState::Starting;
        let mut child = Command::new(&self.binary)
            .arg("serve")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
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
                self.state = RuntimeProcessState::Running;
                self.child = Some(child);
                info!(path = %self.binary.display(), "embedded runtime started");
                Ok(())
            }
            Err(err) => {
                self.state = RuntimeProcessState::Failed;
                Err(RuntimeError::Process(err.to_string()))
            }
        }
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

    #[test]
    fn resolves_binary_path() {
        let supervisor = RuntimeSupervisor::new("/tmp/aisec");
        assert!(supervisor
            .binary_path()
            .ends_with(if cfg!(windows) { "ollama.exe" } else { "ollama" }));
    }
}
