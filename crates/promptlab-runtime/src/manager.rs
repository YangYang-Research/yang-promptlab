//! Central orchestrator for the remote-oriented AI runtime host.

use std::path::PathBuf;

use promptlab_storage::Database;
use serde::{Deserialize, Serialize};

use crate::config::RuntimeConfig;
use crate::error::{RuntimeError, RuntimeResult};
use crate::hardware::{HardwareDetector, RuntimeHardwareProfile};
use crate::logs::{RuntimeLogEntry, RuntimeLogs};
use crate::state::{transition, RuntimeLifecycleState};
use crate::supervisor::{RuntimeProcessState, RuntimeSupervisor};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHealthReport {
    pub lifecycle_state: String,
    pub process_alive: bool,
    pub endpoint_reachable: bool,
    pub latency_ms: u64,
    pub memory_bytes: Option<u64>,
    pub gpu_memory_bytes: Option<u64>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatusSnapshot {
    pub lifecycle_state: String,
    pub runtime_version: Option<String>,
    pub backend: Option<String>,
    pub platform: Option<String>,
    pub install_path: Option<String>,
    pub installed: bool,
    pub verified: bool,
    pub base_url: String,
    pub message: String,
    pub requires_attention: bool,
    pub last_error: Option<String>,
}

pub struct RuntimeManager {
    data_dir: PathBuf,
    db: Option<Database>,
    lifecycle: RuntimeLifecycleState,
    hardware: Option<RuntimeHardwareProfile>,
    supervisor: RuntimeSupervisor,
    logs: RuntimeLogs,
    last_health: Option<RuntimeHealthReport>,
    last_error: Option<String>,
}

impl RuntimeManager {
    pub fn new(data_dir: impl Into<PathBuf>, db: Option<Database>) -> Self {
        let data_dir = data_dir.into();
        let config = RuntimeConfig::new(&data_dir);
        Self {
            data_dir,
            db,
            lifecycle: RuntimeLifecycleState::NotInstalled,
            hardware: None,
            supervisor: RuntimeSupervisor::with_config(config),
            logs: RuntimeLogs::new(500),
            last_health: None,
            last_error: None,
        }
    }

    fn hardware_detector(&self) -> HardwareDetector {
        match &self.db {
            Some(db) => HardwareDetector::with_db(&self.data_dir, db.clone()),
            None => HardwareDetector::new(&self.data_dir),
        }
    }

    pub fn requires_attention(&self) -> bool {
        matches!(
            self.lifecycle,
            RuntimeLifecycleState::Failed | RuntimeLifecycleState::NotInstalled
        )
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn lifecycle_state(&self) -> RuntimeLifecycleState {
        self.lifecycle
    }

    pub fn supervisor(&self) -> &RuntimeSupervisor {
        &self.supervisor
    }

    pub fn supervisor_mut(&mut self) -> &mut RuntimeSupervisor {
        &mut self.supervisor
    }

    pub fn hardware(&self) -> Option<&RuntimeHardwareProfile> {
        self.hardware.as_ref()
    }

    pub fn last_health(&self) -> Option<&RuntimeHealthReport> {
        self.last_health.as_ref()
    }

    pub async fn bootstrap(&mut self) -> RuntimeResult<()> {
        self.log("info", "loading remote-oriented runtime configuration")
            .await;
        self.last_error = None;
        self.hardware = self.hardware_detector().load().await?;

        if let Some(profile) = self.hardware.clone() {
            self.supervisor.set_hardware_profile(profile);
        }

        // Drop leftover local-runtime install marker if present.
        let legacy_manifest = self.data_dir.join("runtime").join("manifest.json");
        if legacy_manifest.is_file() {
            let _ = tokio::fs::remove_file(&legacy_manifest).await;
        }

        self.lifecycle = RuntimeLifecycleState::Installed;
        self.log("info", "runtime host ready (remote providers / Ollama HTTP)")
            .await;
        Ok(())
    }

    pub async fn repair(
        &mut self,
        mut progress: impl FnMut(&str, &str, u8),
    ) -> RuntimeResult<()> {
        self.last_error = None;
        if let Err(err) = self.repair_steps(&mut progress).await {
            self.record_failure(err).await;
            return Err(RuntimeError::NativeRuntimeError(
                self.last_error
                    .clone()
                    .unwrap_or_else(|| "repair failed".into()),
            ));
        }
        Ok(())
    }

    async fn repair_steps(
        &mut self,
        progress: &mut impl FnMut(&str, &str, u8),
    ) -> RuntimeResult<()> {
        progress("hardware", "Detecting hardware profile…", 20);
        self.log("info", "repair: detecting hardware").await;
        let profile = self.hardware_detector().detect_and_persist().await?;
        self.hardware = Some(profile.clone());
        self.supervisor.set_hardware_profile(profile);

        progress("runtime", "Initializing runtime host…", 60);
        self.lifecycle = RuntimeLifecycleState::Installed;
        self.last_error = None;
        progress("complete", "Runtime host ready", 100);
        self.log("info", "repair: runtime host initialized").await;
        Ok(())
    }

    pub fn recommended_runtime_label(&self) -> Option<String> {
        if self.hardware.is_some() {
            Some("remote".into())
        } else {
            None
        }
    }

    pub fn is_runtime_active(&self) -> bool {
        matches!(
            self.lifecycle,
            RuntimeLifecycleState::Running
                | RuntimeLifecycleState::Starting
                | RuntimeLifecycleState::Busy
        )
    }

    pub async fn install(&mut self) -> RuntimeResult<()> {
        self.repair(|_, _, _| {}).await
    }

    pub async fn start_runtime(&mut self) -> RuntimeResult<()> {
        self.lifecycle = transition(self.lifecycle, RuntimeLifecycleState::Starting);
        self.supervisor.ensure_running().await?;
        self.sync_lifecycle_from_supervisor();
        self.log(
            "info",
            format!("runtime started ({})", self.lifecycle.as_str()),
        )
        .await;
        Ok(())
    }

    pub fn sync_lifecycle_from_supervisor(&mut self) {
        if self.supervisor.state() == RuntimeProcessState::Running {
            self.lifecycle = transition(self.lifecycle, RuntimeLifecycleState::Running);
        } else if matches!(self.lifecycle, RuntimeLifecycleState::Stopped) {
        } else {
            self.lifecycle = transition(self.lifecycle, RuntimeLifecycleState::Installed);
        }
    }

    pub async fn stop_runtime(&mut self) -> RuntimeResult<()> {
        self.lifecycle = transition(self.lifecycle, RuntimeLifecycleState::Stopping);
        self.supervisor.stop().await?;
        self.lifecycle = transition(self.lifecycle, RuntimeLifecycleState::Stopped);
        self.log("info", "runtime stopped").await;
        Ok(())
    }

    pub async fn restart_runtime(&mut self) -> RuntimeResult<()> {
        self.supervisor_mut().restart().await?;
        self.sync_lifecycle_from_supervisor();
        self.log("info", "runtime restarted").await;
        self.run_health_check().await?;
        Ok(())
    }

    pub async fn delete_runtime(&mut self) -> RuntimeResult<()> {
        if self.is_runtime_active() {
            self.stop_runtime().await?;
        }

        let legacy_manifest = self.data_dir.join("runtime").join("manifest.json");
        if legacy_manifest.is_file() {
            tokio::fs::remove_file(&legacy_manifest)
                .await
                .map_err(|err| RuntimeError::NativeRuntimeError(err.to_string()))?;
        }

        self.last_health = None;
        self.last_error = None;
        self.lifecycle = RuntimeLifecycleState::NotInstalled;
        self.log("info", "runtime configuration cleared").await;
        Ok(())
    }

    /// Load persisted hardware profile, detecting only when no profile exists yet.
    pub async fn ensure_hardware_profile(&mut self) -> RuntimeResult<RuntimeHardwareProfile> {
        self.log("info", "loading hardware profile").await;
        let profile = self.hardware_detector().ensure_profile().await?;
        self.supervisor.set_hardware_profile(profile.clone());
        self.hardware = Some(profile.clone());
        Ok(profile)
    }

    /// Full hardware re-detection (Reinitialize Engine / explicit refresh only).
    pub async fn refresh_hardware(&mut self) -> RuntimeResult<RuntimeHardwareProfile> {
        self.log("info", "refreshing hardware profile").await;
        let profile = self.hardware_detector().detect_and_persist().await?;
        self.supervisor.set_hardware_profile(profile.clone());
        self.hardware = Some(profile.clone());
        Ok(profile)
    }

    pub async fn run_health_check(&mut self) -> RuntimeResult<RuntimeHealthReport> {
        let started = std::time::Instant::now();
        let healthy = self.supervisor.check_health().await.unwrap_or(false);
        let latency_ms = started.elapsed().as_millis() as u64;
        let runtime_alive = self.supervisor.is_process_alive_async().await;

        let message = if healthy {
            "runtime host ready (remote-only)".into()
        } else if runtime_alive {
            "runtime host starting".into()
        } else {
            "runtime host stopped".into()
        };

        let report = RuntimeHealthReport {
            lifecycle_state: self.lifecycle.as_str().to_string(),
            process_alive: runtime_alive,
            endpoint_reachable: healthy,
            latency_ms,
            memory_bytes: None,
            gpu_memory_bytes: None,
            message,
        };
        self.last_health = Some(report.clone());
        Ok(report)
    }

    pub async fn logs(&self, limit: usize) -> Vec<RuntimeLogEntry> {
        self.logs.entries(limit).await
    }

    pub fn status_snapshot(&self) -> RuntimeStatusSnapshot {
        let installed = !matches!(self.lifecycle, RuntimeLifecycleState::NotInstalled);
        RuntimeStatusSnapshot {
            lifecycle_state: self.lifecycle.as_str().to_string(),
            runtime_version: None,
            backend: Some("remote".into()),
            platform: Some(std::env::consts::OS.to_string()),
            install_path: None,
            installed,
            verified: installed,
            base_url: "remote".into(),
            message: self.status_message(),
            requires_attention: self.requires_attention(),
            last_error: self.last_error.clone(),
        }
    }

    pub async fn status_snapshot_async(&self) -> RuntimeStatusSnapshot {
        self.status_snapshot()
    }

    fn status_message(&self) -> String {
        match self.lifecycle {
            RuntimeLifecycleState::Running => {
                "Runtime host ready — use a remote provider or Ollama over HTTP".into()
            }
            RuntimeLifecycleState::Installed => {
                "Runtime host ready — configure a remote model".into()
            }
            RuntimeLifecycleState::NotInstalled => "AI runtime not configured".into(),
            RuntimeLifecycleState::Starting => "Starting AI runtime…".into(),
            RuntimeLifecycleState::Stopping => "Stopping AI runtime…".into(),
            RuntimeLifecycleState::Stopped => "AI runtime stopped".into(),
            RuntimeLifecycleState::Busy => "AI runtime busy".into(),
            RuntimeLifecycleState::Failed => "AI runtime failed — check configuration".into(),
            RuntimeLifecycleState::Downloading | RuntimeLifecycleState::Installing => {
                "Initializing runtime host…".into()
            }
            RuntimeLifecycleState::Updating => "Updating runtime configuration…".into(),
        }
    }

    async fn log(&self, level: &str, message: impl Into<String>) {
        self.logs.push(level, message).await;
    }

    async fn record_failure(&mut self, err: RuntimeError) {
        self.lifecycle = RuntimeLifecycleState::Failed;
        self.last_error = Some(err.to_string());
        self.log("error", self.last_error.as_ref().expect("last_error"))
            .await;
    }
}
