use promptlab_attack::PayloadAttempt;
use serde::{Deserialize, Serialize};

// Batch checkpoint for deferred pause between attack and judge batches (persisted in playbook_json).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanBatchCheckpoint {
    PendingJudge {
        category: String,
        attempts: Vec<PayloadAttempt>,
    },
    JudgingPartial {
        category: String,
        attempts: Vec<PayloadAttempt>,
        next_judge_index: usize,
    },
}
