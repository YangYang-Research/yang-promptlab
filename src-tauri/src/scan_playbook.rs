//! Persist scan progress and batch checkpoints in `playbook_json`.

use promptlab_storage::{Repositories, ScanRepository, UpdateScan};
use serde_json::Value;

use crate::jobs::{ScanBatchCheckpoint, ScanProgress};

pub fn progress_from_playbook(playbook_json: Option<&str>) -> Option<ScanProgress> {
    let raw = playbook_json?;
    let value: Value = serde_json::from_str(raw).ok()?;
    serde_json::from_value(value.get("progress")?.clone()).ok()
}

pub fn checkpoint_from_playbook(playbook_json: Option<&str>) -> Option<ScanBatchCheckpoint> {
    let raw = playbook_json?;
    let value: Value = serde_json::from_str(raw).ok()?;
    serde_json::from_value(value.get("batch_checkpoint")?.clone()).ok()
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
            obj.insert(
                "progress".into(),
                serde_json::to_value(progress).unwrap_or(Value::Null),
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
