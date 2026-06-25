//! Start/stop/restart the embedded libllama runtime.

use std::path::Path;

use time::OffsetDateTime;

use crate::error::{RuntimeError, RuntimeResult};
use crate::manifest::RuntimeManifest;
use crate::state::{transition, RuntimeLifecycleState};
use crate::supervisor::RuntimeSupervisor;

pub struct RuntimeLauncher;

impl RuntimeLauncher {
    pub async fn start(
        supervisor: &mut RuntimeSupervisor,
        manifest: &mut RuntimeManifest,
        lifecycle: RuntimeLifecycleState,
    ) -> RuntimeResult<RuntimeLifecycleState> {
        if !supervisor.runtime_available() {
            return Err(RuntimeError::Unavailable);
        }

        let state = transition(lifecycle, RuntimeLifecycleState::Starting);
        supervisor.ensure_running().await?;
        manifest.last_started = Some(OffsetDateTime::now_utc());
        Ok(transition(state, RuntimeLifecycleState::Running))
    }

    pub async fn stop(
        supervisor: &mut RuntimeSupervisor,
        lifecycle: RuntimeLifecycleState,
    ) -> RuntimeResult<RuntimeLifecycleState> {
        let state = transition(lifecycle, RuntimeLifecycleState::Stopping);
        supervisor.stop().await?;
        Ok(transition(state, RuntimeLifecycleState::Stopped))
    }

    pub async fn restart(
        supervisor: &mut RuntimeSupervisor,
        manifest: &mut RuntimeManifest,
        lifecycle: RuntimeLifecycleState,
    ) -> RuntimeResult<RuntimeLifecycleState> {
        let _ = Self::stop(supervisor, lifecycle).await?;
        Self::start(supervisor, manifest, RuntimeLifecycleState::Stopped).await
    }

    pub async fn load_model_for_inference(
        supervisor: &mut RuntimeSupervisor,
        model_path: &Path,
        lifecycle: RuntimeLifecycleState,
    ) -> RuntimeResult<RuntimeLifecycleState> {
        let state = transition(lifecycle, RuntimeLifecycleState::Starting);
        supervisor.ensure_model_loaded(model_path).await?;
        Ok(transition(state, RuntimeLifecycleState::Running))
    }
}
