use std::time::Duration;

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager};
use tokio::time::sleep;
use tracing::{info, warn};

use crate::state::AppState;

const WATCH_INTERVAL: Duration = Duration::from_secs(5);

static WATCH_STARTED: AtomicBool = AtomicBool::new(false);

pub fn spawn_runtime_watch(app: AppHandle) {
    if WATCH_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        run_runtime_watch(app).await;
    });
}

async fn run_runtime_watch(app: AppHandle) {
    loop {
        sleep(WATCH_INTERVAL).await;

        let Some(state) = app.try_state::<AppState>() else {
            continue;
        };

        let should_watch = {
            let manager = state.runtime_manager().lock().await;
            manager.supervisor().should_watch()
        };
        if !should_watch {
            continue;
        }

        let mut manager = state.runtime_manager().lock().await;
        let alive = manager.supervisor_mut().is_process_alive();
        let healthy = if alive {
            manager.supervisor_mut().check_health().await.unwrap_or(false)
        } else {
            false
        };

        if !alive || !healthy {
            warn!("embedded runtime unhealthy; restarting");
            if let Err(err) = manager.restart_runtime().await {
                warn!(error = %err, "embedded runtime auto-restart failed");
            } else {
                info!("embedded runtime auto-restarted");
            }
        }
    }
}
