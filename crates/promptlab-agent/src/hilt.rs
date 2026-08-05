//! Human-in-the-loop (HILT) gate for mutating Yazg tools.
//!
//! Create / update / delete tools must not apply side effects until the user
//! approves in the chat UI. The tool returns `status=pending_approval`; the host
//! stores the pending action and resumes via `yazg_resolve_hilt`.
//!
//! Pending actions expire after [`HILT_TTL_SECS`] (15 minutes).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

/// How long a pending mutating action stays approvable.
pub const HILT_TTL_SECS: u64 = 15 * 60;

/// Mutation class used by the chat confirm card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HiltMutationKind {
    Create,
    Update,
    Delete,
}

impl HiltMutationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }
}

/// A mutating tool call waiting for explicit user approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HiltPendingAction {
    pub id: String,
    pub tool: String,
    pub kind: HiltMutationKind,
    /// Validated tool arguments (already trimmed / checked by the tool).
    pub args: Value,
    /// Short human-readable line for the confirm card.
    pub summary: String,
    /// Unix epoch milliseconds when the pending action was created.
    pub created_at_ms: u64,
    /// Unix epoch milliseconds when the pending action expires (auto-deny).
    pub expires_at_ms: u64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl HiltPendingAction {
    pub fn new(
        tool: impl Into<String>,
        kind: HiltMutationKind,
        args: Value,
        summary: impl Into<String>,
    ) -> Self {
        let tool = tool.into();
        let created_at_ms = now_ms();
        let expires_at_ms = created_at_ms.saturating_add(HILT_TTL_SECS.saturating_mul(1000));
        Self {
            id: format!("hilt-{tool}-{created_at_ms}"),
            tool,
            kind,
            args,
            summary: summary.into(),
            created_at_ms,
            expires_at_ms,
        }
    }

    pub fn is_expired(&self) -> bool {
        now_ms() >= self.expires_at_ms
    }

    /// Fallback when the model returns an empty reply after pending_approval.
    pub fn pending_user_reply(&self) -> String {
        if let Some(name) = self.args.get("name").and_then(|v| v.as_str()) {
            return format!("Project {name} đang chờ phê duyệt.");
        }
        format!("{} đang chờ phê duyệt.", self.summary)
    }
}

/// True for tools that write / update / delete workspace state.
/// Extend this list when new mutating tools are added.
pub fn is_mutating_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "create_project"
        // Future: "update_project" | "delete_project" | "create_target" | …
    )
}

/// Mutation kind for a known mutating tool name.
pub fn mutation_kind_for_tool(tool_name: &str) -> Option<HiltMutationKind> {
    match tool_name {
        "create_project" => Some(HiltMutationKind::Create),
        name if name.starts_with("create_") => Some(HiltMutationKind::Create),
        name if name.starts_with("update_") => Some(HiltMutationKind::Update),
        name if name.starts_with("delete_") => Some(HiltMutationKind::Delete),
        _ => None,
    }
}
