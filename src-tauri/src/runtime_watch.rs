use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tokio::time::sleep;
use tracing::{info, warn};

use crate::state::AppState;

const WATCH_INTERVAL: Duration = Duration::from_secs(5);
/// Avoid restarting on a single transient health blip (large models can be slow to respond).
const UNHEALTHY_RESTART_AFTER: u32 = 3;

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
    let mut unhealthy_streak = 0u32;

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
            unhealthy_streak = 0;
            continue;
        }

        let mut manager = state.runtime_manager().lock().await;
        let alive = manager
            .supervisor()
            .is_process_alive_async()
            .await;
        let healthy = if alive {
            manager.supervisor_mut().check_health().await.unwrap_or(false)
        } else {
            false
        };

        if alive && healthy {
            unhealthy_streak = 0;
            continue;
        }

        unhealthy_streak += 1;
        warn!(
            alive,
            healthy,
            streak = unhealthy_streak,
            threshold = UNHEALTHY_RESTART_AFTER,
            "embedded runtime health check failed"
        );

        if unhealthy_streak < UNHEALTHY_RESTART_AFTER {
            continue;
        }
        unhealthy_streak = 0;

        warn!("embedded runtime unhealthy; restarting with model reload");
        if let Err(err) = manager.restart_runtime().await {
            warn!(error = %err, "embedded runtime auto-restart failed");
        } else {
            info!("embedded runtime auto-restarted");
        }
    }
}
