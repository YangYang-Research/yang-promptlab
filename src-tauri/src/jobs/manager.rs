use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub status: String,
    pub current_endpoint: Option<String>,
    pub current_test: Option<String>,
    pub completed: u64,
    pub total: u64,
    pub findings: u64,
    pub started_at: Option<String>,
    #[serde(default)]
    pub agent_mode: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_phase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_attempt: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_retry: Option<u32>,
}

impl ScanProgress {
    pub fn new(total: u64) -> Self {
        Self {
            status: "running".into(),
            current_endpoint: None,
            current_test: None,
            completed: 0,
            total,
            findings: 0,
            started_at: Some(
                OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default(),
            ),
            agent_mode: false,
            current_phase: None,
            current_attempt: None,
            current_retry: None,
        }
    }

    pub fn progress_percent(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.completed as f64 / self.total as f64) * 100.0
    }
}

pub struct JobHandle {
    pub cancel: Arc<AtomicBool>,
    pub paused: Arc<AtomicBool>,
    pub progress: Arc<Mutex<ScanProgress>>,
}

#[derive(Default, Clone)]
pub struct ScanJobManager {
    inner: Arc<Mutex<HashMap<String, JobHandle>>>,
}

impl ScanJobManager {
    pub fn register(
        &self,
        scan_id: String,
        cancel: Arc<AtomicBool>,
        paused: Arc<AtomicBool>,
        progress: Arc<Mutex<ScanProgress>>,
    ) {
        self.inner.lock().unwrap().insert(
            scan_id,
            JobHandle {
                cancel,
                paused,
                progress,
            },
        );
    }

    pub fn remove(&self, scan_id: &str) {
        self.inner.lock().unwrap().remove(scan_id);
    }

    pub fn contains(&self, scan_id: &str) -> bool {
        self.inner.lock().unwrap().contains_key(scan_id)
    }

    pub fn progress(&self, scan_id: &str) -> Option<ScanProgress> {
        self.inner
            .lock()
            .unwrap()
            .get(scan_id)
            .and_then(|handle| handle.progress.lock().ok().map(|p| p.clone()))
    }

    pub fn is_cancelled(&self, scan_id: &str) -> bool {
        self.inner
            .lock()
            .unwrap()
            .get(scan_id)
            .map(|handle| handle.cancel.load(Ordering::Relaxed))
            .unwrap_or(false)
    }

    pub fn set_paused(&self, scan_id: &str, paused: bool) -> bool {
        let handles = self.inner.lock().unwrap();
        let Some(handle) = handles.get(scan_id) else {
            return false;
        };
        handle.paused.store(paused, Ordering::Relaxed);
        if let Ok(mut progress) = handle.progress.lock() {
            progress.status = if paused {
                "paused".into()
            } else {
                "running".into()
            };
        }
        true
    }

    pub fn request_cancel(&self, scan_id: &str) -> bool {
        if let Some(handle) = self.inner.lock().unwrap().get(scan_id) {
            handle.cancel.store(true, Ordering::Relaxed);
            handle.paused.store(false, Ordering::Relaxed);
            if let Ok(mut progress) = handle.progress.lock() {
                progress.status = "cancelled".into();
            }
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_update_progress() {
        let manager = ScanJobManager::default();
        let cancel = Arc::new(AtomicBool::new(false));
        let paused = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Mutex::new(ScanProgress::new(4)));
        manager.register("scan-1".into(), cancel.clone(), paused.clone(), progress.clone());

        {
            let mut p = progress.lock().unwrap();
            p.completed = 2;
            p.findings = 1;
            p.current_endpoint = Some("https://example.com/v1/chat".into());
        }

        let snapshot = manager.progress("scan-1").unwrap();
        assert_eq!(snapshot.completed, 2);
        assert_eq!(snapshot.findings, 1);
        assert_eq!(snapshot.total, 4);
        assert!((snapshot.progress_percent() - 50.0).abs() < f64::EPSILON);

        assert!(manager.set_paused("scan-1", true));
        assert!(paused.load(Ordering::Relaxed));
        assert_eq!(manager.progress("scan-1").unwrap().status, "paused");

        assert!(manager.set_paused("scan-1", false));
        assert!(!paused.load(Ordering::Relaxed));
        assert_eq!(manager.progress("scan-1").unwrap().status, "running");

        manager.request_cancel("scan-1");
        assert!(cancel.load(Ordering::Relaxed));
        assert_eq!(manager.progress("scan-1").unwrap().status, "cancelled");
    }
}
