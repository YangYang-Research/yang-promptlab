//! Central orchestrator for the embedded AI inference runtime.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::benchmark::{RuntimeBenchmark, RuntimeBenchmarkResult};
use crate::config::RuntimeConfig;
use crate::error::{RuntimeError, RuntimeResult};
use crate::hardware::{HardwareDetector, RuntimeHardwareProfile};
use crate::installer::RuntimeInstaller;
use crate::launcher::RuntimeLauncher;
use crate::logs::{RuntimeLogEntry, RuntimeLogs};
use crate::manifest::RuntimeManifest;
use crate::monitor::{RuntimeHealthReport, RuntimeMonitor};
use crate::state::{transition, RuntimeLifecycleState};
use crate::supervisor::RuntimeSupervisor;

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
    pub binary_available: bool,
    pub base_url: String,
    pub model_loaded: bool,
    pub loaded_model_path: Option<String>,
    pub message: String,
}

pub struct RuntimeManager {
    data_dir: PathBuf,
    lifecycle: RuntimeLifecycleState,
    manifest: Option<RuntimeManifest>,
    hardware: Option<RuntimeHardwareProfile>,
    supervisor: RuntimeSupervisor,
    logs: RuntimeLogs,
    last_health: Option<RuntimeHealthReport>,
    last_benchmark: Option<RuntimeBenchmarkResult>,
    bundled_binary: Option<PathBuf>,
}

impl RuntimeManager {
    pub fn new(data_dir: impl Into<PathBuf>, bundled_binary: Option<PathBuf>) -> Self {
        let data_dir = data_dir.into();
        let config = RuntimeConfig::new("", &data_dir);
        Self {
            data_dir,
            lifecycle: RuntimeLifecycleState::NotInstalled,
            manifest: None,
            hardware: None,
            supervisor: RuntimeSupervisor::with_config(config),
            logs: RuntimeLogs::new(500),
            last_health: None,
            last_benchmark: None,
            bundled_binary,
        }
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

    pub fn manifest(&self) -> Option<&RuntimeManifest> {
        self.manifest.as_ref()
    }

    pub fn hardware(&self) -> Option<&RuntimeHardwareProfile> {
        self.hardware.as_ref()
    }

    pub fn last_health(&self) -> Option<&RuntimeHealthReport> {
        self.last_health.as_ref()
    }

    pub fn last_benchmark(&self) -> Option<&RuntimeBenchmarkResult> {
        self.last_benchmark.as_ref()
    }

    pub async fn bootstrap(&mut self) -> RuntimeResult<()> {
        self.log("info", "bootstrapping AI runtime").await;
        self.manifest = RuntimeManifest::load(&self.data_dir).await?;

        if self.manifest.as_ref().is_some_and(|m| m.installed && m.install_path.is_file()) {
            self.lifecycle = RuntimeLifecycleState::Installed;
            self.log("info", "runtime manifest loaded — skipping hardware detection").await;
            self.hardware = HardwareDetector::new(&self.data_dir).load().await?;
        } else {
            self.lifecycle = transition(self.lifecycle, RuntimeLifecycleState::Downloading);
            self.log("info", "first launch — detecting hardware").await;
            let detector = HardwareDetector::new(&self.data_dir);
            self.hardware = Some(detector.detect_and_persist().await?);

            self.lifecycle = transition(self.lifecycle, RuntimeLifecycleState::Installing);
            let installer = RuntimeInstaller::new(&self.data_dir, self.bundled_binary.clone());
            let manifest = installer
                .install(
                    self.hardware.as_ref().expect("hardware profile"),
                    |msg| info!(step = msg, "runtime install"),
                )
                .await?;
            self.manifest = Some(manifest);
            self.lifecycle = RuntimeLifecycleState::Installed;
            self.log("info", "runtime installed and verified").await;
        }

        self.apply_manifest_to_supervisor().await?;
        self.start_runtime().await?;
        self.run_health_check().await?;
        Ok(())
    }

    pub async fn install(&mut self) -> RuntimeResult<()> {
        self.log("info", "runtime install requested").await;
        let detector = HardwareDetector::new(&self.data_dir);
        self.hardware = Some(detector.detect_and_persist().await?);
        self.lifecycle = transition(self.lifecycle, RuntimeLifecycleState::Downloading);
        self.lifecycle = transition(self.lifecycle, RuntimeLifecycleState::Installing);

        let installer = RuntimeInstaller::new(&self.data_dir, self.bundled_binary.clone());
        let manifest = installer
            .install(
                self.hardware.as_ref().expect("hardware"),
                |msg| info!(step = msg, "runtime install"),
            )
            .await?;
        self.manifest = Some(manifest);
        self.lifecycle = RuntimeLifecycleState::Installed;
        self.apply_manifest_to_supervisor().await?;
        self.log("info", "runtime install complete").await;
        Ok(())
    }

    pub async fn start_runtime(&mut self) -> RuntimeResult<()> {
        if !self.supervisor.binary_available() {
            self.lifecycle = RuntimeLifecycleState::Failed;
            return Err(RuntimeError::Unavailable);
        }
        let manifest = self
            .manifest
            .as_mut()
            .ok_or_else(|| RuntimeError::Config("runtime manifest missing".into()))?;
        self.lifecycle = RuntimeLauncher::start(&mut self.supervisor, manifest, self.lifecycle).await?;
        self.log("info", format!("runtime started ({})", self.lifecycle.as_str()))
            .await;
        Ok(())
    }

    pub async fn stop_runtime(&mut self) -> RuntimeResult<()> {
        self.lifecycle = RuntimeLauncher::stop(&mut self.supervisor, self.lifecycle).await?;
        self.log("info", "runtime stopped").await;
        Ok(())
    }

    pub async fn restart_runtime(&mut self) -> RuntimeResult<()> {
        let manifest = self
            .manifest
            .as_mut()
            .ok_or_else(|| RuntimeError::Config("runtime manifest missing".into()))?;
        self.lifecycle =
            RuntimeLauncher::restart(&mut self.supervisor, manifest, self.lifecycle).await?;
        self.log("info", "runtime restarted").await;
        self.run_health_check().await?;
        Ok(())
    }

    pub async fn refresh_hardware(&mut self) -> RuntimeResult<RuntimeHardwareProfile> {
        self.log("info", "refreshing hardware profile").await;
        let profile = HardwareDetector::new(&self.data_dir)
            .detect_and_persist()
            .await?;
        self.hardware = Some(profile.clone());
        Ok(profile)
    }

    pub async fn run_health_check(&mut self) -> RuntimeResult<RuntimeHealthReport> {
        let report = RuntimeMonitor::check(
            &mut self.supervisor,
            self.lifecycle.as_str(),
        )
        .await?;
        self.last_health = Some(report.clone());
        Ok(report)
    }

    pub async fn run_benchmark(&mut self) -> RuntimeResult<RuntimeBenchmarkResult> {
        self.lifecycle = transition(self.lifecycle, RuntimeLifecycleState::Busy);
        let result = RuntimeBenchmark::run(&self.supervisor).await;
        self.lifecycle = transition(self.lifecycle, RuntimeLifecycleState::Running);
        let result = result?;
        self.last_benchmark = Some(result.clone());
        self.log(
            "info",
            format!(
                "benchmark: {:.1} tok/s, {} ms",
                result.tokens_per_sec, result.latency_ms
            ),
        )
        .await;
        Ok(result)
    }

    pub async fn logs(&self, limit: usize) -> Vec<RuntimeLogEntry> {
        self.logs.entries(limit).await
    }

    pub fn status_snapshot(&self) -> RuntimeStatusSnapshot {
        let manifest = self.manifest.as_ref();
        RuntimeStatusSnapshot {
            lifecycle_state: self.lifecycle.as_str().to_string(),
            runtime_version: manifest.map(|m| m.runtime_version.clone()),
            backend: manifest.map(|m| m.backend.as_str().to_string()),
            platform: manifest.map(|m| m.platform.clone()),
            install_path: manifest.map(|m| m.install_path.display().to_string()),
            installed: manifest.is_some_and(|m| m.installed),
            verified: manifest.is_some_and(|m| m.verified),
            binary_available: self.supervisor.binary_available(),
            base_url: self.supervisor.base_url().to_string(),
            model_loaded: self.supervisor.llama_runtime().is_loaded(),
            loaded_model_path: None,
            message: self.status_message(),
        }
    }

    pub async fn status_snapshot_async(&self) -> RuntimeStatusSnapshot {
        let mut snap = self.status_snapshot();
        snap.loaded_model_path = self
            .supervisor
            .llama_runtime()
            .loaded_model_path()
            .await
            .map(|p| p.display().to_string());
        snap
    }

    async fn apply_manifest_to_supervisor(&mut self) -> RuntimeResult<()> {
        let path = self
            .manifest
            .as_ref()
            .map(|m| m.install_path.clone())
            .filter(|p| p.is_file())
            .ok_or(RuntimeError::Unavailable)?;
        self.supervisor.set_binary(path).await
    }

    fn status_message(&self) -> String {
        match self.lifecycle {
            RuntimeLifecycleState::Running => "AI runtime is running".into(),
            RuntimeLifecycleState::Installed => {
                "AI runtime installed — llama-server idle until model activation".into()
            }
            RuntimeLifecycleState::NotInstalled => "AI runtime not installed".into(),
            RuntimeLifecycleState::Downloading => "Downloading AI runtime…".into(),
            RuntimeLifecycleState::Installing => "Installing AI runtime…".into(),
            RuntimeLifecycleState::Starting => "Starting AI runtime…".into(),
            RuntimeLifecycleState::Stopping => "Stopping AI runtime…".into(),
            RuntimeLifecycleState::Stopped => "AI runtime stopped".into(),
            RuntimeLifecycleState::Busy => "AI runtime busy (benchmark)".into(),
            RuntimeLifecycleState::Updating => "Updating AI runtime…".into(),
            RuntimeLifecycleState::Failed => "AI runtime failed — retry install".into(),
        }
    }

    async fn log(&self, level: &str, message: impl Into<String>) {
        self.logs.push(level, message).await;
    }
}
