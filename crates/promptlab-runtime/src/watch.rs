use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::supervisor::RuntimeSupervisor;

const WATCH_INTERVAL: Duration = Duration::from_secs(5);

/// Background task: restart the embedded runtime if the process exits or health checks fail.
pub async fn run_supervisor_watch(supervisor: Arc<Mutex<RuntimeSupervisor>>) {
    loop {
        sleep(WATCH_INTERVAL).await;

        let mut sup = supervisor.lock().await;
        if !sup.should_watch() {
            continue;
        }

        if !sup.is_process_alive() {
            warn!("embedded runtime process exited; restarting");
            if let Err(err) = sup.restart().await {
                warn!(error = %err, "embedded runtime auto-restart failed");
            } else {
                info!("embedded runtime auto-restarted after process exit");
            }
            continue;
        }

        if !sup.check_health().await.unwrap_or(false) {
            warn!("embedded runtime health check failed; restarting");
            if let Err(err) = sup.restart().await {
                warn!(error = %err, "embedded runtime health restart failed");
            } else {
                info!("embedded runtime auto-restarted after health failure");
            }
        }
    }
}
