//! Persist scan progress and batch checkpoints in `playbook_json`.

use promptlab_storage::{Repositories, ScanRepository, UpdateScan};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use promptlab_attack::{AttackCategory, AttackPayload};

use crate::jobs::{ScanBatchCheckpoint, ScanProgress};

const MAX_SCAN_RETRIES: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScanRetryRecord {
    pub at: String,
    pub mode: String,
}

pub fn scan_retries_from_playbook(playbook_json: Option<&str>) -> Vec<ScanRetryRecord> {
    let raw = playbook_json.unwrap_or("");
    let value: Value = serde_json::from_str(raw).unwrap_or(Value::Null);
    value
        .get("scan_retries")
        .and_then(|retries| serde_json::from_value(retries.clone()).ok())
        .unwrap_or_default()
}

/// Record a Retry Scan / retry-failed-categories event on the playbook.
pub fn append_scan_retry(playbook: &mut Value, mode: &str) {
    let at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "now".into());
    let entry = serde_json::json!({ "at": at, "mode": mode });
    let Some(obj) = playbook.as_object_mut() else {
        return;
    };
    let retries = obj
        .entry("scan_retries")
        .or_insert_with(|| serde_json::json!([]));
    let Some(list) = retries.as_array_mut() else {
        return;
    };
    list.push(entry);
    if list.len() > MAX_SCAN_RETRIES {
        let drop = list.len() - MAX_SCAN_RETRIES;
        list.drain(0..drop);
    }
}

pub fn progress_from_playbook(playbook_json: Option<&str>) -> Option<ScanProgress> {
    let raw = playbook_json?;
    let value: Value = serde_json::from_str(raw).ok()?;
    let mut progress: ScanProgress = serde_json::from_value(value.get("progress")?.clone()).ok()?;
    progress.normalize_phase_trail();
    Some(progress)
}

pub fn checkpoint_from_playbook(playbook_json: Option<&str>) -> Option<ScanBatchCheckpoint> {
    let raw = playbook_json?;
    let value: Value = serde_json::from_str(raw).ok()?;
    serde_json::from_value(value.get("batch_checkpoint")?.clone()).ok()
}

pub fn generated_payloads_from_playbook(
    playbook_json: Option<&str>,
) -> Option<std::collections::HashMap<AttackCategory, Vec<AttackPayload>>> {
    let raw = playbook_json?;
    let value: Value = serde_json::from_str(raw).ok()?;
    let payloads: std::collections::HashMap<AttackCategory, Vec<AttackPayload>> =
        serde_json::from_value(value.get("generated_payloads")?.clone()).ok()?;
    if payloads.values().all(|items| items.is_empty()) {
        return None;
    }
    Some(payloads)
}

pub async fn persist_generated_payloads(
    repos: &Repositories,
    scan_id: &str,
    payloads: &std::collections::HashMap<AttackCategory, Vec<AttackPayload>>,
) -> Result<(), promptlab_core::PromptLabError> {
    let scan = repos.scans().get(scan_id).await?;
    let mut playbook = scan
        .playbook_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(obj) = playbook.as_object_mut() {
        obj.insert(
            "generated_payloads".into(),
            serde_json::to_value(payloads).unwrap_or(Value::Null),
        );
    }
    repos
        .scans()
        .update(
            scan_id,
            UpdateScan {
                playbook_json: Some(playbook),
                ..Default::default()
            },
        )
        .await?;
    Ok(())
}

pub async fn persist_playbook_progress(
    repos: &Repositories,
    scan_id: &str,
    progress: &ScanProgress,
) -> Result<(), promptlab_core::PromptLabError> {
    persist_scan_playbook_state(repos, scan_id, Some(progress), None).await
}

pub async fn persist_playbook_checkpoint(
    repos: &Repositories,
    scan_id: &str,
    checkpoint: Option<&ScanBatchCheckpoint>,
) -> Result<(), promptlab_core::PromptLabError> {
    persist_scan_playbook_state(repos, scan_id, None, Some(checkpoint)).await
}

/// Update progress and/or batch checkpoint in the scan playbook.
/// Pass `None` for either field to leave that field unchanged.
pub async fn persist_scan_playbook_state(
    repos: &Repositories,
    scan_id: &str,
    progress: Option<&ScanProgress>,
    batch_checkpoint: Option<Option<&ScanBatchCheckpoint>>,
) -> Result<(), promptlab_core::PromptLabError> {
    let scan = repos.scans().get(scan_id).await?;
    let mut playbook = scan
        .playbook_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    if let Some(obj) = playbook.as_object_mut() {
        if let Some(progress) = progress {
            let mut snapshot = progress.clone();
            snapshot.normalize_phase_trail();
            obj.insert(
                "progress".into(),
                serde_json::to_value(&snapshot).unwrap_or(Value::Null),
            );
        }
        if let Some(checkpoint) = batch_checkpoint {
            match checkpoint {
                Some(cp) => {
                    obj.insert(
                        "batch_checkpoint".into(),
                        serde_json::to_value(cp).unwrap_or(Value::Null),
                    );
                }
                None => {
                    obj.remove("batch_checkpoint");
                }
            }
        }
    }

    repos
        .scans()
        .update(
            scan_id,
            UpdateScan {
                playbook_json: Some(playbook),
                ..Default::default()
            },
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_scan_retry_records_continue_events() {
        let mut playbook = serde_json::json!({ "profile": "standard" });
        append_scan_retry(&mut playbook, "continue");
        append_scan_retry(&mut playbook, "failed_categories");

        let retries = scan_retries_from_playbook(Some(&playbook.to_string()));
        assert_eq!(retries.len(), 2);
        assert_eq!(retries[0].mode, "continue");
        assert_eq!(retries[1].mode, "failed_categories");
        assert!(!retries[0].at.is_empty());
    }

    #[test]
    fn scan_retries_from_playbook_empty_when_missing() {
        assert!(scan_retries_from_playbook(None).is_empty());
        assert!(scan_retries_from_playbook(Some("{}")).is_empty());
    }

    #[test]
    fn generated_payloads_from_playbook_roundtrip() {
        let payload = AttackPayload::new("p1", "Probe", AttackCategory::Jailbreak, "ignore rules");
        let mut pack = std::collections::HashMap::new();
        pack.insert(AttackCategory::Jailbreak, vec![payload]);
        let playbook = serde_json::json!({
            "generated_payloads": pack,
        });
        let restored = generated_payloads_from_playbook(Some(&playbook.to_string())).unwrap();
        assert_eq!(restored.get(&AttackCategory::Jailbreak).unwrap().len(), 1);
        assert!(generated_payloads_from_playbook(Some("{}")).is_none());
    }
}
