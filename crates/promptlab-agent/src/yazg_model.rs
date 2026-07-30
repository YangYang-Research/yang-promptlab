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
use serde_json::json;
use tracing::{info, warn};

use crate::types::{AgentEvent, AgentEventKind, AgentId};
use crate::yazg_tools::SharedYazgState;

/// Default text when a completion is empty (also used for MaxTurns recovery).
pub const EMPTY_FALLBACK_REPLY: &str =
    "I'm Yazg, PromptLab's AI assistant for authorized AI security testing. How can I help?";

/// Pretty-print a stage payload for Thinking / agents.log (full body, no truncation).
pub fn format_stage_payload(stage: &str, body: serde_json::Value) -> String {
    let value = json!({ "stage": stage, "body": body });
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
}

fn truncate_stage(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let shortened: String = t.chars().take(max.saturating_sub(1)).collect();
    format!("{shortened}…")
}
#[derive(Clone)]
pub struct YazgModel {
    llm: Arc<dyn PlannerLlm>,
    /// Optional sink for per-completion request/response stage events (UI Thinking).
    stage_sink: Option<SharedYazgState>,
}

impl YazgModel {
    pub fn new(llm: Arc<dyn PlannerLlm>) -> Self {
        Self {
            llm,
            stage_sink: None,
        }
    }

    pub fn with_stage_sink(mut self, sink: SharedYazgState) -> Self {
        self.stage_sink = Some(sink);
        self
    }

    async fn emit_stage(&self, kind: AgentEventKind, message: impl Into<String>) {
        let message = message.into();
        info!(
            agent = "yazg",
            kind = kind.as_str(),
            message = %truncate_stage(&message, 2_000),
            "yazg stage"
        );
        let event = match kind {
            AgentEventKind::Llm => AgentEvent::llm(AgentId::Yazg, message),
            AgentEventKind::ToolCall => AgentEvent::tool_call(AgentId::Yazg, message),
            AgentEventKind::React => AgentEvent::react(AgentId::Yazg, message),
            AgentEventKind::Info => AgentEvent::info(AgentId::Yazg, message),
            AgentEventKind::Failed => AgentEvent::failed(AgentId::Yazg, message),
            AgentEventKind::Started => AgentEvent::started(AgentId::Yazg, message),
            AgentEventKind::Completed => AgentEvent::completed(AgentId::Yazg, message),
        };
        if let Some(sink) = &self.stage_sink {
            sink.lock().await.artifacts.events.push(event);
        }
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
            stage_sink: None,
        }
    }

    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse<Self::Response>, CompletionError> {
        // OpenAI-style split: system role vs user transcript (never embed System: in user).
        let (mut system, prompt) = split_completion_request(&request);

        match request.tool_choice.as_ref() {
            Some(ToolChoice::Required) => {
                append_system_constraint(
                    &mut system,
                    "You MUST call exactly one tool now. \
                     Do not answer with plain text. Do not claim a tool already ran.",
                );
            }
            Some(ToolChoice::Specific { function_names }) if !function_names.is_empty() => {
                append_system_constraint(
                    &mut system,
                    &format!(
                        "You MUST call tool `{}` now. \
                         Do not answer with plain text. Do not claim a tool already ran.",
                        function_names.join("` or `")
                    ),
                );
            }
            _ => {}
        }

        let tools = request
            .tools
            .iter()
            .map(|tool| ToolSpec::new(&tool.name, &tool.description, tool.parameters.clone()))
            .collect::<Vec<_>>();

        let system_ref = system.as_deref();
        // Wire-shaped payload (what HostYazgLlm maps to OpenAI chat/completions).
        let request_body = json!({
            "messages": [
                { "role": "system", "content": system_ref },
                { "role": "user", "content": prompt },
            ],
            "model_params": {
                "temperature": request.temperature,
                "max_tokens": request.max_tokens,
                "tool_choice": request.tool_choice,
            },
            "tools": tools.iter().map(|t| json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                }
            })).collect::<Vec<_>>(),
        });
        self.emit_stage(
            AgentEventKind::Llm,
            format_stage_payload("llm_request", request_body),
        )
        .await;

        let outcome = if tools.is_empty() {
            let content = self
                .llm
                .complete_with_system(system_ref, &prompt)
                .await
                .map_err(|err| CompletionError::ProviderError(err.to_string()))?;
            ensure_non_empty_text(normalize_completion_text(LlmCompletion::from_text(content)))
        } else {
            let raw = self
                .llm
                .complete_with_tools_and_system(system_ref, &prompt, &tools)
                .await
                .map_err(|err| CompletionError::ProviderError(err.to_string()))?;
            let cleaned = normalize_completion_text(sanitize_completion(raw, &tools));
            if completion_is_empty(&cleaned) {
                // Models often invent tools or return null content on chat turns
                // when tools are bound (tool_choice=auto). Fall back to text-only
                // like a plain Rig agent without tools.
                warn!("empty tool-aware completion; retrying text-only");
                let mut retry_system = system.clone();
                append_system_constraint(
                    &mut retry_system,
                    "Reply in markdown or plain text only. \
                     Do not call tools. Do not emit JSON, tool envelopes, or assistant_reply.",
                );
                self.emit_stage(
                    AgentEventKind::Llm,
                    format_stage_payload(
                        "llm_request_retry_text_only",
                        json!({
                            "messages": [
                                { "role": "system", "content": retry_system },
                                { "role": "user", "content": prompt },
                            ],
                        }),
                    ),
                )
                .await;
                match self
                    .llm
                    .complete_with_system(retry_system.as_deref(), &prompt)
                    .await
                {
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

        self.emit_stage(
            AgentEventKind::Llm,
            format_stage_payload(
                "llm_response",
                json!({
                    "content": raw.content,
                    "tool_calls": raw.tool_calls,
                }),
            ),
        )
        .await;

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
    let unwrapped = unwrap_user_facing_text(raw);
    let stripped = strip_agent_meta_reasoning(&unwrapped);
    // Models often wrap the real answer in ```markdown … ``` after narrating a decision.
    let unfenced = strip_markdown_fence(&stripped);
    if unfenced != stripped {
        return strip_agent_meta_reasoning(&unfenced);
    }
    // Fence may sit after a decision preamble on earlier lines.
    if let Some(body) = extract_fenced_body(&stripped) {
        return strip_agent_meta_reasoning(&body);
    }
    stripped
}

/// True when a reply still leaks tool/ReAct/internal routing text to the user.
pub fn reply_looks_like_agent_meta(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.contains("[tool_call")
        || lower.contains("[tool_result")
        || lower.contains("[reasoning]")
        || lower.contains("react check:")
        || lower.contains("identical tool call")
        || lower.contains("here is the final reply")
        || lower.contains("you already have workspace observation")
        || lower.contains("do not repeat the same tool")
        || lower.contains("only call a different tool")
        || lower.contains("no forced action")
        || lower.contains("observation answers the")
        || lower.contains("observation does not answer")
        || lower.contains("the observation is relevant")
        || lower.contains("the observation is irrelevant")
        || lower.contains("decide whether the observation")
}

fn strip_agent_meta_reasoning(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Prefer explicit final-answer markers when the model narrates then answers.
    for marker in [
        "Here is the final reply:",
        "here is the final reply:",
        "Final Answer:",
        "Final answer:",
        "final reply:",
        "Final reply:",
    ] {
        if let Some(idx) = trimmed.find(marker) {
            let after = trimmed[idx + marker.len()..].trim();
            if !after.is_empty() && !reply_looks_like_agent_meta(after) {
                return after.to_string();
            }
            if !after.is_empty() {
                return strip_agent_meta_lines(after);
            }
        }
    }

    strip_agent_meta_lines(trimmed)
}

fn strip_agent_meta_lines(s: &str) -> String {
    let mut out = Vec::new();
    for line in s.lines() {
        let t = line.trim();
        if t.is_empty() {
            if out.last().is_some_and(|prev: &&str| !prev.is_empty()) {
                out.push("");
            }
            continue;
        }
        if t.starts_with("[tool_call")
            || t.starts_with("[tool_result")
            || t.starts_with("[reasoning]")
            || t.starts_with("ReAct check:")
            || (t == "---" && s.contains("ReAct check:"))
        {
            continue;
        }
        let lower = t.to_lowercase();
        if lower.contains("identical tool call")
            || lower.contains("you already have workspace observation")
            || lower.contains("do not repeat the same tool")
            || lower.contains("only call a different tool")
            || lower.contains("since the tool call is identical")
            || lower.contains("we can skip it")
            || lower.contains("therefore, we can reply")
            || lower.contains("the user has already asked")
            || lower.contains("the user's original question")
            || lower.contains("which translates to")
            || lower.contains("list_targets tool has")
            || lower.contains("has already been called")
            || lower.contains("answer the user in markdown now")
            || lower.contains("respond to the user with a short markdown")
            || lower.contains("do not mention tools")
            || lower.contains("no forced action")
            || lower.starts_with("here is the final reply")
            || lower.contains("observation answers the")
            || lower.contains("observation does not answer")
            || lower.contains("the observation is relevant")
            || lower.contains("the observation is irrelevant")
            || lower.starts_with("decide whether the observation")
            || (lower.starts_with("yes,") && lower.contains("observation"))
            || (lower.starts_with("no,") && lower.contains("observation"))
            || (lower.starts_with("yes.") && lower.contains("observation"))
        {
            continue;
        }
        out.push(line);
    }
    // Trim leading/trailing blank lines introduced by stripping.
    while out.first().is_some_and(|l| l.trim().is_empty()) {
        out.remove(0);
    }
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    out.join("\n").trim().to_string()
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

/// Pull the first fenced ```…``` body when the model narrates then fences the answer.
fn extract_fenced_body(s: &str) -> Option<String> {
    let start = s.find("```")?;
    let after_open = &s[start + 3..];
    let after_lang = after_open
        .trim_start_matches(|c: char| c.is_ascii_alphanumeric())
        .trim_start_matches('\n');
    let end = after_lang.find("```")?;
    let body = after_lang[..end].trim();
    if body.is_empty() {
        None
    } else {
        Some(body.to_string())
    }
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

/// Split Rig [`CompletionRequest`] into OpenAI-style system + user transcript.
///
/// System comes from `preamble` and any `Message::System` in `chat_history`.
/// User prompt is only user/assistant/tool turns (+ documents) — never `System:` text.
fn split_completion_request(request: &CompletionRequest) -> (Option<String>, String) {
    let mut system_parts: Vec<String> = Vec::new();
    if let Some(preamble) = request
        .preamble
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        system_parts.push(preamble.to_string());
    }

    let mut transcript = String::new();
    for message in request.chat_history.iter() {
        match message {
            Message::System { content } => {
                let content = content.trim();
                if content.is_empty() {
                    continue;
                }
                if !system_parts.iter().any(|existing| existing == content) {
                    system_parts.push(content.to_string());
                }
            }
            Message::User { content } => {
                transcript.push_str("User:\n");
                transcript.push_str(&flatten_user_content(content));
                transcript.push_str("\n\n");
            }
            Message::Assistant { content, .. } => {
                transcript.push_str("Assistant:\n");
                transcript.push_str(&flatten_assistant_content(content));
                transcript.push_str("\n\n");
            }
        }
    }
    for doc in &request.documents {
        transcript.push_str("Context:\n");
        transcript.push_str(&doc.to_string());
        transcript.push('\n');
    }

    let system = if system_parts.is_empty() {
        None
    } else {
        Some(system_parts.join("\n\n"))
    };
    (system, transcript.trim().to_string())
}

fn append_system_constraint(system: &mut Option<String>, constraint: &str) {
    let constraint = constraint.trim();
    if constraint.is_empty() {
        return;
    }
    match system {
        Some(existing) => {
            existing.push_str("\n\n");
            existing.push_str(constraint);
        }
        None => *system = Some(constraint.to_string()),
    }
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
    fn system_stays_out_of_user_prompt() {
        use rig::completion::message::UserContent;
        let history = OneOrMany::many(vec![
            Message::System {
                content: "You are Yazg preamble".into(),
            },
            Message::User {
                content: OneOrMany::one(UserContent::text("hi")),
            },
        ])
        .expect("history");
        let request = CompletionRequest {
            model: None,
            preamble: None,
            chat_history: history,
            documents: Vec::new(),
            tools: Vec::new(),
            temperature: None,
            max_tokens: None,
            tool_choice: None,
            additional_params: None,
            output_schema: None,
        };
        let (system, prompt) = split_completion_request(&request);
        assert_eq!(system.as_deref(), Some("You are Yazg preamble"));
        assert!(prompt.starts_with("User:\nhi"), "prompt={prompt}");
        assert!(
            !prompt.contains("System:") && !prompt.contains("You are Yazg preamble"),
            "system leaked into user prompt: {prompt}"
        );
    }

    #[test]
    fn preamble_and_system_message_deduped() {
        use rig::completion::message::UserContent;
        let history = OneOrMany::many(vec![
            Message::System {
                content: "Same system".into(),
            },
            Message::User {
                content: OneOrMany::one(UserContent::text("hi")),
            },
        ])
        .expect("history");
        let request = CompletionRequest {
            model: None,
            preamble: Some("Same system".into()),
            chat_history: history,
            documents: Vec::new(),
            tools: Vec::new(),
            temperature: None,
            max_tokens: None,
            tool_choice: None,
            additional_params: None,
            output_schema: None,
        };
        let (system, _) = split_completion_request(&request);
        assert_eq!(system.as_deref(), Some("Same system"));
    }

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

    #[test]
    fn strips_tool_call_and_react_meta_from_reply() {
        let raw = r#"[tool_call id=call-1 name=list_targets args={"project":"AI"}]

The user has already asked for the list of targets in project AI, and the list_targets tool has been called with the same arguments. Since the tool call is identical, we can skip it and reply to the user directly.

You already have workspace Observation(s). If they answer the user, reply in markdown now (Finish). Do not repeat the same tool.

Here is the final reply:

### Targets in AI
1. **10.100.109.76** (`019fa414`) — llm_api
2. **192.168.30.146** (`019f99fe`) — llm_api"#;
        let cleaned = normalize_user_facing_reply(raw);
        assert!(!cleaned.contains("[tool_call"));
        assert!(!cleaned.contains("identical"));
        assert!(!cleaned.contains("Observation"));
        assert!(cleaned.contains("10.100.109.76"));
        assert!(cleaned.contains("192.168.30.146"));
        assert!(!reply_looks_like_agent_meta(&cleaned));
    }

    #[test]
    fn strips_observation_decision_and_markdown_fence() {
        let raw = "Yes, the observation answers the user's question.\n\n```markdown\nProjects:\n  - id=abc name=AI targets=2\n```";
        let cleaned = normalize_user_facing_reply(raw);
        assert!(!cleaned.to_lowercase().contains("observation answers"));
        assert!(!cleaned.contains("```"));
        assert!(cleaned.contains("name=AI") || cleaned.contains("AI"));
    }
}
