//! CompletionModel adapter over PromptLab's `PlannerLlm` / AI Runtime.
//!
//! Maps Rig [`CompletionRequest`] like provider adapters in Rig examples:
//! preamble → system, chat_history → user/assistant turns, tools → tool specs.

use std::collections::HashSet;
use std::sync::Arc;

use promptlab_planner::{LlmCompletion, PlannerLlm, ToolCall as PlannerToolCall, ToolSpec};
use rig::OneOrMany;
use rig::completion::{
    AssistantContent, CompletionError, CompletionModel, CompletionRequest, CompletionResponse,
    GetTokenUsage, Message, Usage,
};
use rig::message::{ToolCall, ToolChoice, ToolFunction, UserContent};
use rig::streaming::StreamingCompletionResponse;
use serde::{Deserialize, Serialize};
use tracing::warn;

const EMPTY_FALLBACK_REPLY: &str =
    "I'm Yazg, PromptLab's AI assistant for authorized AI security testing. How can I help?";

/// Cloneable model that delegates completions to PromptLab inference.
#[derive(Clone)]
pub struct YazgModel {
    llm: Arc<dyn PlannerLlm>,
}

impl YazgModel {
    pub fn new(llm: Arc<dyn PlannerLlm>) -> Self {
        Self { llm }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct YazgRawResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<promptlab_planner::ToolCall>,
}

impl GetTokenUsage for YazgRawResponse {
    fn token_usage(&self) -> Usage {
        Usage::new()
    }
}

impl CompletionModel for YazgModel {
    type Response = YazgRawResponse;
    type StreamingResponse = YazgRawResponse;
    type Client = ();

    fn make(_: &Self::Client, _: impl Into<String>) -> Self {
        // Placeholder; production paths always construct via `new`.
        Self {
            llm: Arc::new(UnsupportedLlm),
        }
    }

    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<Self::Response>, CompletionError> {
        // Rig OpenAI adapter: preamble is system; chat_history is the transcript.
        let system = request
            .preamble
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let mut prompt = flatten_chat_history(&request);

        match request.tool_choice.as_ref() {
            Some(ToolChoice::Required) => {
                prompt.push_str(
                    "\n\nSystem constraint: You MUST call exactly one tool now. \
                     Do not answer with plain text. Do not claim a tool already ran.",
                );
            }
            Some(ToolChoice::Specific { function_names }) if !function_names.is_empty() => {
                prompt.push_str(&format!(
                    "\n\nSystem constraint: You MUST call tool `{}` now. \
                     Do not answer with plain text. Do not claim a tool already ran.",
                    function_names.join("` or `")
                ));
            }
            _ => {}
        }

        let tools = request
            .tools
            .iter()
            .map(|tool| ToolSpec::new(&tool.name, &tool.description, tool.parameters.clone()))
            .collect::<Vec<_>>();

        let outcome = if tools.is_empty() {
            let content = self
                .llm
                .complete_with_system(system, &prompt)
                .await
                .map_err(|err| CompletionError::ProviderError(err.to_string()))?;
            ensure_non_empty_text(normalize_completion_text(LlmCompletion::from_text(content)))
        } else {
            let raw = self
                .llm
                .complete_with_tools_and_system(system, &prompt, &tools)
                .await
                .map_err(|err| CompletionError::ProviderError(err.to_string()))?;
            let cleaned = normalize_completion_text(sanitize_completion(raw, &tools));
            if completion_is_empty(&cleaned) {
                // Models often invent tools or return null content on chat turns
                // when tools are bound (tool_choice=auto). Fall back to text-only
                // like a plain Rig agent without tools.
                warn!("empty tool-aware completion; retrying text-only");
                let mut retry = prompt.clone();
                retry.push_str(
                    "\n\nSystem constraint: Reply in markdown or plain text only. \
                     Do not call tools. Do not emit JSON, tool envelopes, or assistant_reply.",
                );
                match self.llm.complete_with_system(system, &retry).await {
                    Ok(text) if !text.trim().is_empty() => ensure_non_empty_text(
                        normalize_completion_text(LlmCompletion::from_text(text)),
                    ),
                    Ok(_) | Err(_) => ensure_non_empty_text(cleaned),
                }
            } else {
                cleaned
            }
        };

        let raw = YazgRawResponse {
            content: outcome.content.clone(),
            tool_calls: outcome.tool_calls.clone(),
        };

        let choice = completion_to_assistant_content(&outcome)?;
        Ok(CompletionResponse {
            choice,
            usage: Usage::new(),
            raw_response: raw,
            message_id: None,
        })
    }

    async fn stream(
        &self,
        _request: CompletionRequest,
    ) -> Result<StreamingCompletionResponse<Self::StreamingResponse>, CompletionError> {
        Err(CompletionError::ProviderError(
            "YazgModel does not support streaming yet".into(),
        ))
    }
}

struct UnsupportedLlm;

#[async_trait::async_trait]
impl PlannerLlm for UnsupportedLlm {
    async fn complete(&self, _prompt: &str) -> promptlab_planner::PlannerResult<String> {
        Err(promptlab_planner::PlannerError::Llm(
            "YazgModel placeholder has no LLM bound".into(),
        ))
    }
}

fn completion_is_empty(outcome: &LlmCompletion) -> bool {
    let text_empty = outcome
        .content
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_none();
    text_empty && outcome.tool_calls.is_empty()
}

fn ensure_non_empty_text(mut outcome: LlmCompletion) -> LlmCompletion {
    if completion_is_empty(&outcome) {
        outcome.content = Some(EMPTY_FALLBACK_REPLY.into());
    }
    outcome
}

/// Unwrap text that looks like a fake tool envelope, e.g.
/// `{"name":"assistant_reply","parameters":{"text":"..."}}`.
fn normalize_completion_text(mut outcome: LlmCompletion) -> LlmCompletion {
    if let Some(content) = outcome.content.take() {
        outcome.content = Some(unwrap_user_facing_text(&content));
    }
    outcome
}

/// Public unwrap for final chat replies (tool-envelope / quoted / JS-concat junk).
pub fn normalize_user_facing_reply(raw: &str) -> String {
    unwrap_user_facing_text(raw)
}

fn unwrap_user_facing_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Strip a single outer quote pair if the whole reply is quoted.
    let unquoted = strip_wrapping_quotes(trimmed);
    let softened = soften_js_string_concat_in_json(&unquoted);

    if let Some(extracted) = extract_text_from_tool_envelope(&softened) {
        return unwrap_js_string_concat(&extracted);
    }

    // Sometimes the model emits the envelope as the whole message with markdown fences.
    let fence_stripped = strip_markdown_fence(&softened);
    if fence_stripped != softened {
        let fence_soft = soften_js_string_concat_in_json(&fence_stripped);
        if let Some(extracted) = extract_text_from_tool_envelope(&fence_soft) {
            return unwrap_js_string_concat(&extracted);
        }
    }

    // Lenient path for invalid JSON envelopes with JS `+` still present.
    if let Some(extracted) = extract_text_from_tool_envelope_lenient(&unquoted) {
        return unwrap_js_string_concat(&extracted);
    }

    unwrap_js_string_concat(&unquoted)
}

/// Merge `"a" + "b"` fragments inside a JSON-ish payload so serde can parse it.
fn soften_js_string_concat_in_json(s: &str) -> String {
    let mut out = s.to_string();
    loop {
        let Some(next) = merge_one_js_string_concat(&out) else {
            break;
        };
        if next == out {
            break;
        }
        out = next;
    }
    out
}

fn merge_one_js_string_concat(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        let (lit1, after1) = scan_json_string(bytes, i)?;
        let mut j = after1;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'+' {
            i = after1;
            continue;
        }
        j += 1;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'"' {
            i = after1;
            continue;
        }
        let (lit2, after2) = scan_json_string(bytes, j)?;
        let mut merged = String::with_capacity(lit1.len() + lit2.len() + 2);
        merged.push('"');
        for ch in lit1.chars().chain(lit2.chars()) {
            match ch {
                '\\' | '"' => {
                    merged.push('\\');
                    merged.push(ch);
                }
                _ => merged.push(ch),
            }
        }
        merged.push('"');
        let mut next = String::with_capacity(s.len());
        next.push_str(&s[..i]);
        next.push_str(&merged);
        next.push_str(&s[after2..]);
        return Some(next);
    }
    None
}

fn scan_json_string(bytes: &[u8], start: usize) -> Option<(String, usize)> {
    if start >= bytes.len() || bytes[start] != b'"' {
        return None;
    }
    let mut i = start + 1;
    let mut lit = String::new();
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() => {
                lit.push(bytes[i + 1] as char);
                i += 2;
            }
            b'"' => return Some((lit, i + 1)),
            b => {
                lit.push(b as char);
                i += 1;
            }
        }
    }
    None
}

fn extract_text_from_tool_envelope_lenient(s: &str) -> Option<String> {
    let lower = s.to_ascii_lowercase();
    if !(lower.contains("assistant_reply")
        || lower.contains("final_answer")
        || lower.contains("\"parameters\"")
        || lower.contains("\"arguments\""))
    {
        return None;
    }
    for key in ["text", "message", "content", "reply", "answer", "response"] {
        let needle = format!("\"{key}\"");
        let Some(idx) = s.find(&needle) else {
            continue;
        };
        let after_key = &s[idx + needle.len()..];
        let Some(colon) = after_key.find(':') else {
            continue;
        };
        let value_region = after_key[colon + 1..].trim_start();
        // Take until we hit `}` at depth 0 for objects, else until end / trailing braces.
        let candidate = take_js_value_expr(value_region);
        let unwrapped = unwrap_js_string_concat(candidate.trim().trim_end_matches(',').trim());
        let cleaned = strip_wrapping_quotes(&unwrapped);
        if !cleaned.is_empty() {
            return Some(cleaned);
        }
    }
    None
}

fn take_js_value_expr(s: &str) -> &str {
    let bytes = s.as_bytes();
    if bytes.first() == Some(&b'"') {
        // Span across `"a" + "b"` until a non-concat token.
        let mut i = 0;
        loop {
            let Some((_, after)) = scan_json_string(bytes, i) else {
                return s;
            };
            let mut j = after;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'+' {
                j += 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'"' {
                    i = j;
                    continue;
                }
            }
            return &s[..after];
        }
    }
    // Fallback: cut at first unmatched closing brace / end.
    if let Some(end) = s.find(['}', '\n']) {
        return s[..end].trim();
    }
    s
}

fn strip_wrapping_quotes(s: &str) -> String {
    let t = s.trim();
    if t.len() >= 2 {
        let bytes = t.as_bytes();
        if (bytes[0] == b'"' && bytes[t.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[t.len() - 1] == b'\'')
        {
            return t[1..t.len() - 1].to_string();
        }
    }
    t.to_string()
}

fn strip_markdown_fence(s: &str) -> String {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix("```") {
        let rest = rest
            .trim_start_matches(|c: char| c.is_ascii_alphanumeric())
            .trim_start_matches('\n');
        if let Some(body) = rest.strip_suffix("```") {
            return body.trim().to_string();
        }
    }
    t.to_string()
}

fn extract_text_from_tool_envelope(s: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(s).ok()?;
    let obj = parsed.as_object()?;
    let name = obj
        .get("name")
        .or_else(|| obj.get("tool"))
        .and_then(|v| {
            if let Some(s) = v.as_str() {
                Some(s.to_string())
            } else {
                v.get("name").and_then(|n| n.as_str()).map(str::to_string)
            }
        })
        .unwrap_or_default();
    if !is_synthetic_reply_tool(&name) {
        return None;
    }
    let args = obj
        .get("parameters")
        .or_else(|| obj.get("arguments"))
        .or_else(|| obj.get("args"))
        .cloned()
        .unwrap_or(parsed.clone());
    extract_reply_text(&args).or_else(|| extract_reply_text(&parsed))
}

/// Collapse trivial JS-style `"a" + "b"` concatenations some models emit inside JSON.
fn unwrap_js_string_concat(s: &str) -> String {
    let t = s.trim();
    if !t.contains('+') || !t.contains('"') {
        return t.to_string();
    }
    // Match sequences of quoted string literals joined by +.
    let mut out = String::new();
    let mut rest = t;
    let mut matched_any = false;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        if !rest.starts_with('"') {
            return t.to_string();
        }
        let bytes = rest.as_bytes();
        let mut i = 1;
        let mut lit = String::new();
        while i < bytes.len() {
            match bytes[i] {
                b'\\' if i + 1 < bytes.len() => {
                    lit.push(bytes[i + 1] as char);
                    i += 2;
                }
                b'"' => {
                    i += 1;
                    break;
                }
                b => {
                    lit.push(b as char);
                    i += 1;
                }
            }
        }
        if i == 1 {
            return t.to_string();
        }
        out.push_str(&lit);
        matched_any = true;
        rest = rest[i..].trim_start();
        if rest.is_empty() {
            break;
        }
        if let Some(next) = rest.strip_prefix('+') {
            rest = next;
            continue;
        }
        return t.to_string();
    }
    if matched_any { out } else { t.to_string() }
}

/// Drop / rewrite invented tool names (e.g. `assistant_reply`) so Rig does not
/// raise `UnknownToolCall` on greetings and plain-text turns.
fn sanitize_completion(mut outcome: LlmCompletion, tools: &[ToolSpec]) -> LlmCompletion {
    if outcome.tool_calls.is_empty() {
        return outcome;
    }
    let allowed: HashSet<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    let mut kept: Vec<PlannerToolCall> = Vec::new();
    let mut reply_bits: Vec<String> = Vec::new();

    for call in outcome.tool_calls.drain(..) {
        if allowed.contains(call.name.as_str()) {
            kept.push(call);
            continue;
        }
        if let Some(text) = extract_reply_text(&call.arguments) {
            reply_bits.push(text);
            continue;
        }
        if is_synthetic_reply_tool(&call.name) {
            warn!(
                tool = %call.name,
                "dropping synthetic reply tool with empty args"
            );
            continue;
        }
        warn!(
            tool = %call.name,
            "dropping unknown tool call (not in bound tool set)"
        );
    }

    if !reply_bits.is_empty() {
        let joined = reply_bits.join("\n").trim().to_string();
        if !joined.is_empty() {
            match outcome.content.as_mut() {
                Some(existing) if !existing.trim().is_empty() => {
                    existing.push_str("\n");
                    existing.push_str(&joined);
                }
                _ => outcome.content = Some(joined),
            }
        }
    }
    outcome.tool_calls = kept;
    outcome
}

fn is_synthetic_reply_tool(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "assistant_reply"
            | "assistant_response"
            | "final_answer"
            | "final_reply"
            | "respond"
            | "reply"
            | "message"
            | "chat"
            | "say"
            | "answer"
    )
}

fn extract_reply_text(arguments: &serde_json::Value) -> Option<String> {
    const KEYS: &[&str] = &[
        "message",
        "reply",
        "text",
        "content",
        "answer",
        "response",
        "utterance",
        "output",
        "body",
    ];
    if let Some(s) = arguments.as_str().map(str::trim).filter(|s| !s.is_empty()) {
        return Some(s.to_string());
    }
    if let Some(obj) = arguments.as_object() {
        for key in KEYS {
            if let Some(s) = obj
                .get(*key)
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                return Some(s.to_string());
            }
            // Nested object: {"reply":{"text":"..."}}
            if let Some(nested) = obj.get(*key) {
                if let Some(s) = extract_reply_text(nested) {
                    return Some(s);
                }
            }
        }
        if obj.len() == 1 {
            if let Some(s) = obj
                .values()
                .next()
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn completion_to_assistant_content(
    outcome: &LlmCompletion,
) -> Result<OneOrMany<AssistantContent>, CompletionError> {
    let mut parts = Vec::new();
    if let Some(content) = outcome
        .content
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        parts.push(AssistantContent::text(content));
    }
    for call in &outcome.tool_calls {
        parts.push(AssistantContent::ToolCall(ToolCall::new(
            call.id.clone(),
            ToolFunction::new(call.name.clone(), call.arguments.clone()),
        )));
    }
    match parts.len() {
        // Avoid Rig's empty-assistant-turn path; callers already ensure text when possible.
        0 => Ok(OneOrMany::one(AssistantContent::text(EMPTY_FALLBACK_REPLY))),
        1 => Ok(OneOrMany::one(parts.remove(0))),
        _ => OneOrMany::many(parts).map_err(|_| {
            CompletionError::ResponseError("failed to build assistant content".into())
        }),
    }
}

/// Flatten Rig chat history into a single prompt string (user/assistant/tool turns).
/// Preamble is passed separately as the API `system` message — same split as Rig's
/// OpenAI `CompletionRequest` adapter.
fn flatten_chat_history(request: &CompletionRequest) -> String {
    let mut out = String::new();
    for message in request.chat_history.iter() {
        match message {
            Message::System { content } => {
                out.push_str("System:\n");
                out.push_str(content);
                out.push_str("\n\n");
            }
            Message::User { content } => {
                out.push_str("User:\n");
                out.push_str(&flatten_user_content(content));
                out.push_str("\n\n");
            }
            Message::Assistant { content, .. } => {
                out.push_str("Assistant:\n");
                out.push_str(&flatten_assistant_content(content));
                out.push_str("\n\n");
            }
        }
    }
    for doc in &request.documents {
        out.push_str("Context:\n");
        out.push_str(&doc.to_string());
        out.push('\n');
    }
    out.trim().to_string()
}

fn flatten_user_content(content: &OneOrMany<UserContent>) -> String {
    let mut parts = Vec::new();
    for item in content.iter() {
        match item {
            UserContent::Text(text) => parts.push(text.text.clone()),
            UserContent::ToolResult(result) => {
                let body = result
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        rig::message::ToolResultContent::Text(t) => Some(t.text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                parts.push(format!("[tool_result id={}]\n{body}", result.id));
            }
            other => parts.push(format!("{other:?}")),
        }
    }
    parts.join("\n")
}

fn flatten_assistant_content(content: &OneOrMany<AssistantContent>) -> String {
    let mut parts = Vec::new();
    for item in content.iter() {
        match item {
            AssistantContent::Text(text) => parts.push(text.text.clone()),
            AssistantContent::ToolCall(call) => parts.push(format!(
                "[tool_call id={} name={} args={}]",
                call.id, call.function.name, call.function.arguments
            )),
            AssistantContent::Reasoning(reasoning) => {
                parts.push(format!("[reasoning] {reasoning:?}"));
            }
            AssistantContent::Image(_) => parts.push("[image]".into()),
        }
    }
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn assistant_reply_becomes_text() {
        let tools = vec![ToolSpec::new(
            "summary",
            "summary tool",
            json!({"type": "object", "properties": {}}),
        )];
        let outcome = LlmCompletion {
            content: None,
            tool_calls: vec![PlannerToolCall {
                id: "1".into(),
                name: "assistant_reply".into(),
                arguments: json!({"message": "Hello!"}),
            }],
        };
        let cleaned = sanitize_completion(outcome, &tools);
        assert!(cleaned.tool_calls.is_empty());
        assert_eq!(cleaned.content.as_deref(), Some("Hello!"));
    }

    #[test]
    fn known_tools_kept() {
        let tools = vec![ToolSpec::new(
            "summary",
            "summary tool",
            json!({"type": "object", "properties": {}}),
        )];
        let outcome = LlmCompletion {
            content: None,
            tool_calls: vec![PlannerToolCall {
                id: "1".into(),
                name: "summary".into(),
                arguments: json!({}),
            }],
        };
        let cleaned = sanitize_completion(outcome, &tools);
        assert_eq!(cleaned.tool_calls.len(), 1);
        assert_eq!(cleaned.tool_calls[0].name, "summary");
    }

    #[test]
    fn unknown_non_reply_tool_dropped() {
        let tools = vec![ToolSpec::new(
            "summary",
            "summary tool",
            json!({"type": "object", "properties": {}}),
        )];
        let outcome = LlmCompletion {
            content: Some("ok".into()),
            tool_calls: vec![PlannerToolCall {
                id: "1".into(),
                name: "hallucinated_tool".into(),
                arguments: json!({}),
            }],
        };
        let cleaned = sanitize_completion(outcome, &tools);
        assert!(cleaned.tool_calls.is_empty());
        assert_eq!(cleaned.content.as_deref(), Some("ok"));
    }

    #[test]
    fn ensure_non_empty_fills_fallback() {
        let filled = ensure_non_empty_text(LlmCompletion::default());
        assert_eq!(filled.content.as_deref(), Some(EMPTY_FALLBACK_REPLY));
    }

    #[test]
    fn unwraps_assistant_reply_json_envelope() {
        let raw = r#"{"name":"assistant_reply","parameters":{"text":"Hello there"}}"#;
        assert_eq!(unwrap_user_facing_text(raw), "Hello there");
    }

    #[test]
    fn unwraps_invalid_json_with_js_concat() {
        let raw = r#"{"name": "assistant_reply", "parameters": {"text": "Your previous question was " + "What is 1 + 1 ?"}}"#;
        assert_eq!(
            unwrap_user_facing_text(raw),
            "Your previous question was What is 1 + 1 ?"
        );
    }

    #[test]
    fn strips_wrapping_quotes() {
        assert_eq!(unwrap_user_facing_text(r#""Hello!""#), "Hello!");
    }
}
