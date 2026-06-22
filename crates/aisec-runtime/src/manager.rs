//! Central orchestrator for the embedded AI inference runtime.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::benchmark::{RuntimeBenchmark, RuntimeBenchmarkResult};
use crate::config::RuntimeConfig;
use crate::error::{RuntimeError, RuntimeResult};
use crate::hardware::{HardwareDetector, RuntimeHardwareProfile};
use crate::installer::RuntimeInstaller;
use crate::launcher::RuntimeLauncher;
use crate::logs::{RuntimeLogEntry, RuntimeLogs};
use crate::manifest::RuntimeManifest;
use crate::monitor::{RuntimeHealthReport, RuntimeMonitor};
use crate::paths::bundled_llama_server_binary;
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
    pub requires_attention: bool,
    pub last_error: Option<String>,
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
    last_error: Option<String>,
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
            last_error: None,
        }
    }

    pub fn requires_attention(&self) -> bool {
        if matches!(
            self.lifecycle,
            RuntimeLifecycleState::NotInstalled
                | RuntimeLifecycleState::Failed
                | RuntimeLifecycleState::Downloading
                | RuntimeLifecycleState::Installing
        ) {
            return true;
        }

        let manifest_ok = self
            .manifest
            .as_ref()
            .is_some_and(|m| m.installed && m.install_path.is_file());
        !manifest_ok || !self.supervisor.binary_available()
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
        self.log("info", "loading runtime configuration").await;
        self.last_error = None;
        self.manifest = RuntimeManifest::load(&self.data_dir).await?;

        // Load persisted hardware profile only — never detect on startup.
        self.hardware = HardwareDetector::new(&self.data_dir).load().await?;

        let binary_path = self.install_path_from_manifest();
        let binary_valid = binary_path.is_file()
            && RuntimeInstaller::validate_binary(&binary_path).await.is_ok();

        if binary_valid {
            self.lifecycle = RuntimeLifecycleState::Installed;
            if let Err(err) = self.apply_manifest_to_supervisor().await {
                self.record_failure(err).await;
                return Ok(());
            }
            self.log("info", "runtime configuration loaded (idle)").await;
            return Ok(());
        }

        let was_marked_installed = self
            .manifest
            .as_ref()
            .is_some_and(|m| m.installed && m.install_path.is_file());
        self.lifecycle = if was_marked_installed {
            RuntimeLifecycleState::Failed
        } else {
            RuntimeLifecycleState::NotInstalled
        };
        let message = if was_marked_installed {
            "AI runtime install is corrupt or incomplete — repair required"
        } else {
            "AI runtime not installed"
        };
        self.last_error = Some(message.into());
        self.log("warn", message).await;
        Ok(())
    }

    /// Full install/reinstall + start pipeline for UI-driven setup.
    pub async fn repair(&mut self, mut progress: impl FnMut(&str, &str, u8)) -> RuntimeResult<()> {
        self.last_error = None;
        if let Err(err) = self.repair_steps(&mut progress).await {
            self.record_failure(err).await;
            return Err(RuntimeError::Process(
                self.last_error.clone().unwrap_or_else(|| "repair failed".into()),
            ));
        }
        Ok(())
    }

    async fn repair_steps(
        &mut self,
        progress: &mut impl FnMut(&str, &str, u8),
    ) -> RuntimeResult<()> {
        progress("hardware", "Detecting hardware profile…", 10);
        self.log("info", "repair: detecting hardware").await;
        let detector = HardwareDetector::new(&self.data_dir);
        self.hardware = Some(detector.detect_and_persist().await?);

        self.lifecycle = transition(self.lifecycle, RuntimeLifecycleState::Downloading);
        self.lifecycle = transition(self.lifecycle, RuntimeLifecycleState::Installing);

        let installer = RuntimeInstaller::new(&self.data_dir, self.bundled_binary.clone());
        let mut emit_install = |msg: &str| {
            let (step, phase) = progress_from_install_msg(msg);
            progress(step, msg, phase);
        };
        let manifest = installer
            .install(
                self.hardware.as_ref().expect("hardware"),
                &mut emit_install,
            )
            .await?;
        self.manifest = Some(manifest);
        self.lifecycle = RuntimeLifecycleState::Installed;
        self.log("info", "repair: runtime installed").await;

        progress("complete", "Runtime installed — press Start Runtime when ready", 100);
        self.log("info", "repair: install complete (runtime not started)").await;
        Ok(())
    }

    pub fn recommended_runtime_label(&self) -> Option<String> {
        let profile = self.hardware.as_ref()?;
        let package = RuntimeInstaller::select_package(profile).ok()?;
        Some(format!("llama.cpp ({})", package.backend.as_str()))
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
        if !self.supervisor.binary_available() {
            self.lifecycle = RuntimeLifecycleState::Failed;
            return Err(RuntimeError::Unavailable);
        }
        let manifest = self
            .manifest
            .as_mut()
            .ok_or_else(|| RuntimeError::Config("runtime manifest missing".into()))?;
        self.lifecycle = RuntimeLauncher::start(&mut self.supervisor, manifest, self.lifecycle).await?;
        self.sync_lifecycle_from_supervisor();
        self.log("info", format!("runtime started ({})", self.lifecycle.as_str()))
            .await;
        Ok(())
    }

    pub fn sync_lifecycle_from_supervisor(&mut self) {
        if self.supervisor.llama_runtime().is_loaded() {
            self.lifecycle = transition(self.lifecycle, RuntimeLifecycleState::Running);
        } else if self.supervisor.binary_available() {
            self.lifecycle = transition(self.lifecycle, RuntimeLifecycleState::Installed);
        }
    }

    pub fn on_model_load_started(&mut self) {
        self.lifecycle = transition(self.lifecycle, RuntimeLifecycleState::Starting);
    }

    pub fn on_model_load_finished(&mut self, ok: bool) {
        self.lifecycle = if ok {
            transition(self.lifecycle, RuntimeLifecycleState::Running)
        } else {
            transition(self.lifecycle, RuntimeLifecycleState::Failed)
        };
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
            requires_attention: self.requires_attention(),
            last_error: self.last_error.clone(),
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
                "AI runtime installed — idle until a model is loaded".into()
            }
            RuntimeLifecycleState::NotInstalled => "AI runtime not installed".into(),
            RuntimeLifecycleState::Downloading => "Downloading AI runtime…".into(),
            RuntimeLifecycleState::Installing => "Installing AI runtime…".into(),
            RuntimeLifecycleState::Starting => {
                "Loading model into llama-server — large models may take several minutes on CPU"
                    .into()
            }
            RuntimeLifecycleState::Stopping => "Stopping AI runtime…".into(),
            RuntimeLifecycleState::Stopped => "AI runtime stopped".into(),
            RuntimeLifecycleState::Busy => "AI runtime busy (benchmark)".into(),
            RuntimeLifecycleState::Updating => "Updating AI runtime…".into(),
            RuntimeLifecycleState::Failed => "AI runtime failed — retry install".into(),
        }
    }

    fn install_path_from_manifest(&self) -> PathBuf {
        self.manifest
            .as_ref()
            .map(|m| m.install_path.clone())
            .filter(|p| p.is_file())
            .unwrap_or_else(|| bundled_llama_server_binary(&self.data_dir))
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

fn progress_from_install_msg(msg: &str) -> (&'static str, u8) {
    match msg {
        "selecting runtime package" => ("package", 20),
        "installing bundled runtime" | "installing development runtime" => ("install", 60),
        "downloading runtime" => ("download", 40),
        "runtime already installed" => ("verify", 70),
        "verifying runtime" => ("verify", 80),
        _ => ("install", 50),
    }
}
