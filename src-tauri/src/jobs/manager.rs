use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::checkpoint::ScanBatchCheckpoint;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub status: String,
    pub current_endpoint: Option<String>,
    pub current_test: Option<String>,
    pub completed: u64,
    pub total: u64,
    /// Count of categories that finished attack + judge successfully.
    /// Equals `categories_succeeded.len()` on current runs; legacy playbooks may
    /// still count failed slots here.
    #[serde(default)]
    pub categories_completed: u64,
    /// Category ids that finished with an error (not counted in `categories_succeeded`).
    #[serde(default)]
    pub categories_failed: Vec<String>,
    /// Category ids that finished attack + judge successfully. Used to skip on Retry Scan.
    #[serde(default)]
    pub categories_succeeded: Vec<String>,
    pub findings: u64,
    pub started_at: Option<String>,
    #[serde(default)]
    pub agent_mode: bool,
    #[serde(default)]
    pub pause_pending: bool,
    /// HTTP requests sent (matches est. requests in attack plan).
    #[serde(default)]
    pub attacks_completed: u64,
    #[serde(default)]
    pub attacks_total: u64,
    /// Enabled test cases from the attack plan (wizard total_testcases).
    #[serde(default)]
    pub testcases_completed: u64,
    #[serde(default)]
    pub testcases_total: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_phase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_attempt: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_retry: Option<u32>,
    /// Live execution trail (e.g. generate → attack → recover → attack → judge).
    /// Appends on each distinct phase transition; capped to bound memory/UI size.
    #[serde(default)]
    pub phase_trail: Vec<String>,
}

impl ScanProgress {
    pub fn new(total: u64) -> Self {
        Self {
            status: "running".into(),
            current_endpoint: None,
            current_test: None,
            completed: 0,
            total,
            categories_completed: 0,
            categories_failed: Vec::new(),
            categories_succeeded: Vec::new(),
            findings: 0,
            started_at: Some(
                OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default(),
            ),
            agent_mode: false,
            pause_pending: false,
            attacks_completed: 0,
            attacks_total: 0,
            testcases_completed: 0,
            testcases_total: 0,
            current_phase: None,
            current_attempt: None,
            current_retry: None,
            phase_trail: Vec::new(),
        }
    }

    pub fn progress_percent(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        let raw = ((self.completed as f64 / self.total as f64) * 100.0).min(100.0);
        // Planned units (est. requests × pipeline) fill before Sequential recovery /
        // auto-retry finishes. Never show 100% while the job is still active.
        match self.status.as_str() {
            "completed" | "failed" | "cancelled" | "stopped" | "error" => raw,
            _ => raw.min(99.0),
        }
    }

    pub fn bump(&mut self, units: u64) {
        self.completed = self.completed.saturating_add(units).min(self.total);
    }

    /// Record a phase transition for the live Execution pipeline trail.
    /// Attack/Judge entries may include a category label as `phase|Category Name`.
    pub fn push_phase(&mut self, phase: &str) {
        self.push_phase_with_category(phase, None);
    }

    pub fn push_phase_with_category(&mut self, phase: &str, category: Option<&str>) {
        const MAX_TRAIL: usize = 64;
        let phase_key = phase.trim().to_ascii_lowercase();
        if phase_key.is_empty() {
            return;
        }
        // Preparing / Generate run once for the whole scan. Later categories (and
        // Retry continue) must not append a second Generate node.
        if matches!(phase_key.as_str(), "preparing" | "generate")
            && self.phase_trail.iter().any(|entry| {
                entry
                    .split('|')
                    .next()
                    .unwrap_or(entry)
                    .eq_ignore_ascii_case(&phase_key)
            })
        {
            return;
        }
        let entry = match category.map(str::trim).filter(|s| !s.is_empty()) {
            Some(cat) if matches!(phase_key.as_str(), "attack" | "judge") => {
                format!("{phase_key}|{cat}")
            }
            _ => phase_key,
        };
        if self.phase_trail.last().map(String::as_str) == Some(entry.as_str()) {
            return;
        }
        self.phase_trail.push(entry);
        if self.phase_trail.len() > MAX_TRAIL {
            let drop = self.phase_trail.len() - MAX_TRAIL;
            self.phase_trail.drain(0..drop);
        }
    }

    /// Update live phase + trail. Does not rewind current_phase to a singleton
    /// stage (preparing/generate) that already ran.
    pub fn apply_phase(
        &mut self,
        phase: &str,
        category: Option<&str>,
        attempt: Option<u32>,
        retry: Option<u32>,
    ) {
        let phase_key = phase.trim().to_ascii_lowercase();
        let singleton = matches!(phase_key.as_str(), "preparing" | "generate");
        let already = self.phase_trail.iter().any(|entry| {
            entry
                .split('|')
                .next()
                .unwrap_or(entry)
                .eq_ignore_ascii_case(&phase_key)
        });
        self.push_phase_with_category(phase, category);
        if singleton && already {
            return;
        }
        if !phase_key.is_empty() {
            self.current_phase = Some(phase_key);
        }
        if let Some(label) = category.map(str::trim).filter(|s| !s.is_empty()) {
            self.current_test = Some(label.to_string());
        }
        self.current_attempt = attempt;
        self.current_retry = retry;
    }

    /// Map finished HTTP requests to enabled test cases (ceil), capped at total.
    pub fn sync_testcases_completed(&mut self) {
        if self.attacks_total == 0 || self.testcases_total == 0 {
            return;
        }
        let scaled = self.attacks_completed.saturating_mul(self.testcases_total);
        let completed = (scaled + self.attacks_total - 1) / self.attacks_total;
        self.testcases_completed = completed.min(self.testcases_total);
    }

    /// Skip this category on continue/retry. Failed and never-started categories are rerun.
    pub fn should_skip_category(&self, category_id: &str, index: usize) -> bool {
        if self.categories_succeeded.iter().any(|id| id == category_id) {
            return true;
        }
        if self.categories_failed.iter().any(|id| id == category_id) {
            return false;
        }
        if !self.categories_succeeded.is_empty() {
            return false;
        }
        (index as u64) < self.categories_completed
    }

    pub fn mark_category_success(&mut self, category_id: &str) {
        self.categories_failed.retain(|id| id != category_id);
        if !self.categories_succeeded.iter().any(|id| id == category_id) {
            self.categories_succeeded.push(category_id.to_string());
        }
        self.categories_completed = self.categories_succeeded.len() as u64;
    }

    pub fn mark_category_failure(&mut self, category_id: &str) {
        self.categories_succeeded.retain(|id| id != category_id);
        if !self.categories_failed.iter().any(|id| id == category_id) {
            self.categories_failed.push(category_id.to_string());
        }
        self.categories_completed = self.categories_succeeded.len() as u64;
    }

    /// Preparing is a scan-level stage. Older playbooks / crash snapshots may omit it
    /// even when later stages were persisted.
    pub fn normalize_phase_trail(&mut self) {
        if self.phase_trail.is_empty() {
            return;
        }
        let has_preparing = self.phase_trail.iter().any(|entry| {
            entry
                .split('|')
                .next()
                .unwrap_or(entry)
                .eq_ignore_ascii_case("preparing")
        });
        if !has_preparing {
            self.phase_trail.insert(0, "preparing".into());
        }
    }
}

pub fn bump_scan_progress(progress: &Arc<Mutex<ScanProgress>>, units: u64) {
    if let Ok(mut state) = progress.lock() {
        state.bump(units);
    }
}

pub struct JobHandle {
    pub cancel: Arc<AtomicBool>,
    pub paused: Arc<AtomicBool>,
    pub pause_requested: Arc<AtomicBool>,
    pub batch_checkpoint: Arc<Mutex<Option<ScanBatchCheckpoint>>>,
    pub progress: Arc<Mutex<ScanProgress>>,
}

#[derive(Clone)]
pub struct ScanJobControls {
    pub cancel: Arc<AtomicBool>,
    pub paused: Arc<AtomicBool>,
    pub pause_requested: Arc<AtomicBool>,
    pub batch_checkpoint: Arc<Mutex<Option<ScanBatchCheckpoint>>>,
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
        pause_requested: Arc<AtomicBool>,
        batch_checkpoint: Arc<Mutex<Option<ScanBatchCheckpoint>>>,
        progress: Arc<Mutex<ScanProgress>>,
    ) {
        self.inner.lock().unwrap().insert(
            scan_id,
            JobHandle {
                cancel,
                paused,
                pause_requested,
                batch_checkpoint,
                progress,
            },
        );
    }

    pub fn controls(&self, scan_id: &str) -> Option<ScanJobControls> {
        self.inner.lock().unwrap().get(scan_id).map(|handle| ScanJobControls {
            cancel: handle.cancel.clone(),
            paused: handle.paused.clone(),
            pause_requested: handle.pause_requested.clone(),
            batch_checkpoint: handle.batch_checkpoint.clone(),
        })
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

    pub fn request_pause(&self, scan_id: &str) -> bool {
        let handles = self.inner.lock().unwrap();
        let Some(handle) = handles.get(scan_id) else {
            return false;
        };
        handle.pause_requested.store(true, Ordering::Relaxed);
        if let Ok(mut progress) = handle.progress.lock() {
            progress.pause_pending = true;
        }
        true
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
            if !paused {
                progress.pause_pending = false;
            }
        }
        true
    }

    pub fn request_cancel(&self, scan_id: &str) -> bool {
        if let Some(handle) = self.inner.lock().unwrap().get(scan_id) {
            handle.cancel.store(true, Ordering::Relaxed);
            handle.paused.store(false, Ordering::Relaxed);
            handle.pause_requested.store(false, Ordering::Relaxed);
            if let Ok(mut progress) = handle.progress.lock() {
                progress.status = "cancelled".into();
                progress.pause_pending = false;
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
        let pause_requested = Arc::new(AtomicBool::new(false));
        let batch_checkpoint = Arc::new(Mutex::new(None));
        let progress = Arc::new(Mutex::new(ScanProgress::new(4)));
        manager.register(
            "scan-1".into(),
            cancel.clone(),
            paused.clone(),
            pause_requested.clone(),
            batch_checkpoint,
            progress.clone(),
        );

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

        assert!(manager.request_pause("scan-1"));
        assert!(pause_requested.load(Ordering::Relaxed));
        assert!(manager.progress("scan-1").unwrap().pause_pending);

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

    #[test]
    fn sync_testcases_completed_maps_requests_to_testcases() {
        let mut progress = ScanProgress::new(100);
        progress.attacks_total = 1200;
        progress.testcases_total = 12;
        progress.attacks_completed = 69;
        progress.sync_testcases_completed();
        assert_eq!(progress.testcases_completed, 1);

        progress.attacks_completed = 1200;
        progress.sync_testcases_completed();
        assert_eq!(progress.testcases_completed, 12);
    }

    #[test]
    fn skip_succeeded_and_rerun_failed_in_the_middle() {
        let mut progress = ScanProgress::new(3);
        progress.mark_category_success("prompt_injection");
        progress.mark_category_failure("jailbreak");
        progress.mark_category_success("system_prompt_extraction");

        assert!(progress.should_skip_category("prompt_injection", 0));
        assert!(!progress.should_skip_category("jailbreak", 1));
        assert!(progress.should_skip_category("system_prompt_extraction", 2));
        assert_eq!(progress.categories_completed, 2);
        assert_eq!(
            progress.categories_succeeded,
            vec!["prompt_injection", "system_prompt_extraction"]
        );
        assert_eq!(progress.categories_failed, vec!["jailbreak"]);
    }

    #[test]
    fn generate_phase_appears_once_in_the_trail() {
        let mut progress = ScanProgress::new(4);
        progress.apply_phase("preparing", None, None, None);
        progress.apply_phase("generate", Some("all categories"), None, None);
        progress.apply_phase("attack", Some("Prompt Injection"), Some(1), None);
        progress.apply_phase("judge", Some("Prompt Injection"), Some(1), None);
        progress.apply_phase("generate", Some("all categories"), None, None);
        progress.apply_phase("attack", Some("Jailbreak"), Some(1), None);

        assert_eq!(
            progress.phase_trail,
            vec![
                "preparing",
                "generate",
                "attack|Prompt Injection",
                "judge|Prompt Injection",
                "attack|Jailbreak",
            ]
        );
        assert_eq!(progress.current_phase.as_deref(), Some("attack"));
        assert_eq!(progress.current_test.as_deref(), Some("Jailbreak"));
    }

    #[test]
    fn normalize_phase_trail_inserts_missing_preparing() {
        let mut progress = ScanProgress::new(4);
        progress.phase_trail = vec![
            "generate".into(),
            "attack|Prompt Injection".into(),
            "judge|Prompt Injection".into(),
        ];
        progress.normalize_phase_trail();
        assert_eq!(progress.phase_trail[0], "preparing");
        assert_eq!(progress.phase_trail.len(), 4);
    }

    #[test]
    fn legacy_skip_uses_completed_cursor_when_succeeded_empty() {
        let mut progress = ScanProgress::new(3);
        progress.categories_completed = 3;
        progress.categories_failed = vec!["jailbreak".into()];

        assert!(progress.should_skip_category("prompt_injection", 0));
        assert!(!progress.should_skip_category("jailbreak", 1));
        assert!(progress.should_skip_category("system_prompt_extraction", 2));
    }

    #[test]
    fn progress_percent_caps_below_100_while_running() {
        let mut progress = ScanProgress::new(4);
        progress.completed = 4;
        progress.status = "running".into();
        assert_eq!(progress.progress_percent(), 99.0);

        progress.status = "completed".into();
        assert_eq!(progress.progress_percent(), 100.0);
    }
}
