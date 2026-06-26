//! Central orchestrator for the embedded AI inference runtime.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::benchmark::{RuntimeBenchmark, RuntimeBenchmarkResult};
use crate::config::RuntimeConfig;
use crate::error::{RuntimeError, RuntimeResult};
use crate::hardware::{HardwareDetector, RuntimeHardwareProfile};
use crate::launcher::RuntimeLauncher;
use crate::local_runtime_adapter::{resolve_backend, GfxBackend};
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
    last_error: Option<String>,
}

impl RuntimeManager {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        let data_dir = data_dir.into();
        let config = RuntimeConfig::new(&data_dir);
        Self {
            data_dir,
            lifecycle: RuntimeLifecycleState::NotInstalled,
            manifest: None,
            hardware: None,
            supervisor: RuntimeSupervisor::with_config(config),
            logs: RuntimeLogs::new(500),
            last_health: None,
            last_benchmark: None,
            last_error: None,
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
        self.log("info", "loading embedded libllama runtime configuration").await;
        self.last_error = None;
        self.manifest = RuntimeManifest::load(&self.data_dir).await?;
        self.hardware = HardwareDetector::new(&self.data_dir).load().await?;

        if let Some(profile) = self.hardware.clone() {
            self.supervisor.set_hardware_profile(profile);
        }

        if self.manifest.is_none() {
            let backend = resolve_backend(GfxBackend::Auto, self.hardware.as_ref());
            let mut manifest = RuntimeManifest::new(
                "embedded-libllama",
                backend,
                std::env::consts::OS,
                crate::paths::runtime_dir(&self.data_dir),
            );
            manifest.installed = true;
            manifest.verified = true;
            manifest.installed_at = Some(OffsetDateTime::now_utc());
            manifest.save(&self.data_dir).await?;
            self.manifest = Some(manifest);
        }

        self.supervisor.local_runtime().initialize().await?;
        self.lifecycle = RuntimeLifecycleState::Installed;
        self.log("info", "embedded libllama runtime ready (no model loaded)").await;
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
                self.last_error.clone().unwrap_or_else(|| "repair failed".into()),
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
        let profile = HardwareDetector::new(&self.data_dir)
            .detect_and_persist()
            .await?;
        self.hardware = Some(profile.clone());
        self.supervisor.set_hardware_profile(profile);

        progress("runtime", "Initializing embedded libllama…", 60);
        self.supervisor.local_runtime().initialize().await?;

        let backend = resolve_backend(GfxBackend::Auto, self.hardware.as_ref());
        let mut manifest = self.manifest.clone().unwrap_or_else(|| {
            RuntimeManifest::new(
                "embedded-libllama",
                backend,
                std::env::consts::OS,
                crate::paths::runtime_dir(&self.data_dir),
            )
        });
        manifest.backend = backend;
        manifest.installed = true;
        manifest.verified = true;
        manifest.installed_at = Some(OffsetDateTime::now_utc());
        manifest.save(&self.data_dir).await?;
        self.manifest = Some(manifest);

        self.lifecycle = RuntimeLifecycleState::Installed;
        self.last_error = None;
        progress("complete", "Embedded libllama runtime ready", 100);
        self.log("info", "repair: embedded runtime initialized").await;
        Ok(())
    }

    pub fn recommended_runtime_label(&self) -> Option<String> {
        if self.hardware.is_some() {
            Some("libllama".into())
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
        if !self.supervisor.runtime_available() {
            self.lifecycle = RuntimeLifecycleState::Failed;
            self.last_error = Some("embedded libllama runtime unavailable".into());
            return Err(RuntimeError::Unavailable);
        }
        let manifest = self
            .manifest
            .as_mut()
            .ok_or_else(|| RuntimeError::Config("runtime manifest missing".into()))?;
        self.lifecycle =
            RuntimeLauncher::start(&mut self.supervisor, manifest, self.lifecycle).await?;
        self.sync_lifecycle_from_supervisor();
        self.log(
            "info",
            format!("runtime started ({})", self.lifecycle.as_str()),
        )
        .await;
        Ok(())
    }

    pub fn sync_lifecycle_from_supervisor(&mut self) {
        use crate::supervisor::RuntimeProcessState;

        if self.supervisor.local_runtime().is_loaded()
            || self.supervisor.state() == RuntimeProcessState::Running
        {
            self.lifecycle = transition(self.lifecycle, RuntimeLifecycleState::Running);
        } else if matches!(self.lifecycle, RuntimeLifecycleState::Stopped) {
        } else if self.supervisor.runtime_available() {
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

    pub async fn load_model_at_path(
        &mut self,
        model_path: &std::path::Path,
    ) -> RuntimeResult<()> {
        if self.is_model_loaded_at(model_path).await {
            self.sync_lifecycle_from_supervisor();
            return Ok(());
        }
        if !matches!(self.lifecycle, RuntimeLifecycleState::Starting) {
            self.on_model_load_started();
        }
        let result = self.supervisor.ensure_model_loaded(model_path).await;
        self.on_model_load_finished(result.is_ok());
        if result.is_ok() {
            self.sync_lifecycle_from_supervisor();
        }
        result
    }

    pub async fn is_same_model_loaded_at(&self, model_path: &std::path::Path) -> bool {
        let adapter = self.supervisor.local_runtime();
        let Some(loaded) = adapter.loaded_model_path().await else {
            return false;
        };
        if !crate::paths::same_paths(&loaded, model_path) {
            return false;
        }
        adapter.is_loaded_async().await
    }

    pub async fn is_model_loaded_at(&mut self, model_path: &std::path::Path) -> bool {
        if !self.is_same_model_loaded_at(model_path).await {
            return false;
        }
        self.supervisor.check_health().await.unwrap_or(false)
    }

    pub async fn unload_loaded_model(&mut self) -> RuntimeResult<()> {
        if self.supervisor.local_runtime().is_loaded() {
            self.supervisor.stop().await?;
        }
        self.lifecycle = RuntimeLifecycleState::Installed;
        Ok(())
    }

    pub async fn stop_runtime(&mut self) -> RuntimeResult<()> {
        self.lifecycle = RuntimeLauncher::stop(&mut self.supervisor, self.lifecycle).await?;
        self.log("info", "runtime stopped").await;
        Ok(())
    }

    pub async fn restart_runtime(&mut self) -> RuntimeResult<()> {
        self.supervisor_mut().restart().await?;
        self.sync_lifecycle_from_supervisor();
        if let Some(manifest) = self.manifest.as_mut() {
            manifest.last_started = Some(OffsetDateTime::now_utc());
        }
        self.log("info", "runtime restarted").await;
        self.run_health_check().await?;
        Ok(())
    }

    pub async fn delete_runtime(&mut self) -> RuntimeResult<()> {
        if self.is_runtime_active() {
            self.stop_runtime().await?;
        }

        let manifest_path = RuntimeManifest::path(&self.data_dir);
        if manifest_path.is_file() {
            tokio::fs::remove_file(&manifest_path)
                .await
                .map_err(|err| RuntimeError::NativeRuntimeError(err.to_string()))?;
        }

        self.manifest = None;
        self.last_health = None;
        self.last_benchmark = None;
        self.last_error = None;
        self.lifecycle = RuntimeLifecycleState::NotInstalled;
        self.log("info", "runtime configuration cleared").await;
        Ok(())
    }

    /// Load persisted hardware profile, detecting only when no profile exists yet.
    pub async fn ensure_hardware_profile(&mut self) -> RuntimeResult<RuntimeHardwareProfile> {
        self.log("info", "loading hardware profile").await;
        let profile = HardwareDetector::new(&self.data_dir)
            .ensure_profile()
            .await?;
        self.supervisor.set_hardware_profile(profile.clone());
        self.hardware = Some(profile.clone());
        Ok(profile)
    }

    /// Full hardware re-detection (Reinitialize Engine / explicit refresh only).
    pub async fn refresh_hardware(&mut self) -> RuntimeResult<RuntimeHardwareProfile> {
        self.log("info", "refreshing hardware profile").await;
        let profile = HardwareDetector::new(&self.data_dir)
            .detect_and_persist()
            .await?;
        self.supervisor.set_hardware_profile(profile.clone());
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
            binary_available: self.supervisor.runtime_available(),
            base_url: "embedded".into(),
            model_loaded: self.supervisor.local_runtime().is_loaded(),
            loaded_model_path: None,
            message: self.status_message(),
            requires_attention: self.requires_attention(),
            last_error: self.last_error.clone(),
        }
    }

    pub async fn status_snapshot_async(&self) -> RuntimeStatusSnapshot {
        let mut snap = self.status_snapshot();
        let adapter = self.supervisor.local_runtime();
        snap.loaded_model_path = adapter
            .loaded_model_path()
            .await
            .map(|p| p.display().to_string());
        snap.model_loaded = adapter.is_loaded_async().await;
        snap
    }

    fn status_message(&self) -> String {
        match self.lifecycle {
            RuntimeLifecycleState::Running => {
                if self.supervisor.local_runtime().is_loaded() {
                    "AI runtime is running".into()
                } else {
                    "AI runtime ready — load a model to start inference".into()
                }
            }
            RuntimeLifecycleState::Installed => {
                "Embedded libllama runtime ready — idle until a model is loaded".into()
            }
            RuntimeLifecycleState::NotInstalled => "AI runtime not configured".into(),
            RuntimeLifecycleState::Starting => {
                "Loading GGUF model via embedded libllama — large models may take several minutes on CPU"
                    .into()
            }
            RuntimeLifecycleState::Stopping => "Stopping AI runtime…".into(),
            RuntimeLifecycleState::Stopped => "AI runtime stopped".into(),
            RuntimeLifecycleState::Busy => "AI runtime busy (benchmark)".into(),
            RuntimeLifecycleState::Failed => "AI runtime failed — reinitialize the engine".into(),
            RuntimeLifecycleState::Downloading | RuntimeLifecycleState::Installing => {
                "Initializing embedded libllama…".into()
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
