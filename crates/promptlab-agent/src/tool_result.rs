//! Uniform agent tool result envelope (Anthropic / OpenAI / Prompting Guide style).
//!
//! Workspace tools return JSON strings of this shape so the model reasons over
//! structured observations — not free-text that the harness string-parses.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Fixed taxonomy for tool misses / failures returned as Ok(JSON) to the agent loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolErrorClass {
    NotFound,
    Validation,
    Empty,
    Skipped,
    Transient,
    Internal,
}

impl ToolErrorClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::Validation => "validation",
            Self::Empty => "empty",
            Self::Skipped => "skipped",
            Self::Transient => "transient",
            Self::Internal => "internal",
        }
    }

    pub fn retryable_default(self) -> bool {
        matches!(self, Self::Transient)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Ok,
    Error,
}

/// Standard tool observation contract for Yazg workspace tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub status: ToolStatus,
    pub tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_class: Option<ToolErrorClass>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hints: Option<Vec<String>>,
}

impl ToolResult {
    pub fn ok(tool: &str, data: impl Serialize) -> Self {
        Self {
            status: ToolStatus::Ok,
            tool: tool.to_string(),
            data: Some(serde_json::to_value(data).unwrap_or(Value::Null)),
            error_class: None,
            message: None,
            retryable: None,
            candidates: None,
            hints: None,
        }
    }

    pub fn error(
        tool: &str,
        class: ToolErrorClass,
        message: impl Into<String>,
        candidates: Option<Vec<Value>>,
        hints: Option<Vec<String>>,
    ) -> Self {
        Self {
            status: ToolStatus::Error,
            tool: tool.to_string(),
            data: None,
            error_class: Some(class),
            message: Some(message.into()),
            retryable: Some(class.retryable_default()),
            candidates,
            hints,
        }
    }

    pub fn not_found(
        tool: &str,
        message: impl Into<String>,
        candidates: Vec<Value>,
        hints: Vec<String>,
    ) -> Self {
        Self::error(
            tool,
            ToolErrorClass::NotFound,
            message,
            Some(candidates),
            Some(hints),
        )
    }

    pub fn skipped(tool: &str, message: impl Into<String>, hints: Vec<String>) -> Self {
        Self::error(
            tool,
            ToolErrorClass::Skipped,
            message,
            None,
            Some(hints),
        )
    }

    pub fn validation(tool: &str, message: impl Into<String>, hints: Vec<String>) -> Self {
        Self::error(
            tool,
            ToolErrorClass::Validation,
            message,
            None,
            Some(hints),
        )
    }

    pub fn empty(tool: &str, message: impl Into<String>, hints: Vec<String>) -> Self {
        Self::error(tool, ToolErrorClass::Empty, message, None, Some(hints))
    }

    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            json!({
                "status": "error",
                "tool": self.tool,
                "error_class": "internal",
                "message": "failed to serialize tool result",
                "retryable": false
            })
            .to_string()
        })
    }

    pub fn parse(s: &str) -> Option<Self> {
        let t = s.trim();
        if !(t.starts_with('{') && t.contains("\"status\"")) {
            return None;
        }
        serde_json::from_str(t).ok()
    }

    pub fn is_ok(&self) -> bool {
        self.status == ToolStatus::Ok
    }

    pub fn is_error(&self) -> bool {
        self.status == ToolStatus::Error
    }

    pub fn is_skipped(&self) -> bool {
        self.error_class == Some(ToolErrorClass::Skipped)
    }

    pub fn is_not_found(&self) -> bool {
        self.error_class == Some(ToolErrorClass::NotFound)
    }

    /// Compact user-facing markdown from an error envelope (salvage / MaxTurns).
    pub fn error_user_markdown(&self) -> Option<String> {
        if !self.is_error() {
            return None;
        }
        let message = self.message.as_deref().unwrap_or("Request failed.").trim();
        let mut out = message.to_string();
        if let Some(cands) = self.candidates.as_ref().filter(|c| !c.is_empty()) {
            let bullets: Vec<String> = cands
                .iter()
                .filter_map(|c| candidate_bullet(c))
                .collect();
            if !bullets.is_empty() {
                out.push_str("\n\nCandidates:\n");
                out.push_str(&bullets.join("\n"));
            }
        }
        Some(out)
    }
}

fn candidate_bullet(v: &Value) -> Option<String> {
    match v {
        Value::Object(map) => {
            let name = map
                .get("name")
                .and_then(|x| x.as_str())
                .or_else(|| map.get("title").and_then(|x| x.as_str()));
            let id = map.get("id").and_then(|x| x.as_str());
            match (name, id) {
                (Some(n), Some(i)) => Some(format!("- **{n}** (`{i}`)")),
                (Some(n), None) => Some(format!("- **{n}**")),
                (None, Some(i)) => Some(format!("- `{i}`")),
                _ => Some(format!("- {v}")),
            }
        }
        Value::String(s) => Some(format!("- {s}")),
        _ => Some(format!("- {v}")),
    }
}

/// One-line note appended to workspace tool descriptions.
pub const TOOL_RESULT_CONTRACT: &str = " Returns JSON {status:ok|error, tool, data?, error_class?, message?, candidates?, hints?, retryable?}.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_roundtrip() {
        let r = ToolResult::ok("list_workspace", json!({"projects": []}));
        let s = r.to_json_string();
        let parsed = ToolResult::parse(&s).expect("parse");
        assert!(parsed.is_ok());
        assert_eq!(parsed.tool, "list_workspace");
        assert!(parsed.data.is_some());
    }

    #[test]
    fn not_found_includes_candidates() {
        let r = ToolResult::not_found(
            "project_detail",
            "No project matching `WebApp`",
            vec![json!({"id": "p1", "name": "AI"})],
            vec!["Ask the user to confirm the project name".into()],
        );
        let s = r.to_json_string();
        let parsed = ToolResult::parse(&s).unwrap();
        assert!(parsed.is_not_found());
        assert_eq!(parsed.retryable, Some(false));
        let md = parsed.error_user_markdown().unwrap();
        assert!(md.contains("WebApp"));
        assert!(md.contains("AI"));
        assert!(md.contains("p1"));
    }

    #[test]
    fn skipped_envelope() {
        let r = ToolResult::skipped(
            "project_detail",
            "`yazg` is the assistant name, not a workspace project",
            vec!["Reply in natural language if the user greeted or asked who you are".into()],
        );
        assert!(r.is_skipped());
        assert!(!r.is_ok());
    }
}
