use std::time::Duration;

use tauri::{AppHandle, Manager};
use tokio::time::sleep;
use tracing::{info, warn};

use crate::state::AppState;

const WATCH_INTERVAL: Duration = Duration::from_secs(5);

pub fn spawn_runtime_watch(app: AppHandle) {
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

        let mut sup = state.runtime_supervisor().lock().await;
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
