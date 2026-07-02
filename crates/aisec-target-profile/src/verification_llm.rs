//! AI Runtime validation of verify HTTP responses.

use aisec_inference::PromptRegistry;
use aisec_planner::PlannerLlm;
use serde::Deserialize;
use time::OffsetDateTime;
use tracing::warn;

use crate::types::{TargetProfile, VerificationResult};
use crate::verification::{
    execute_verify_http, extract_model_from_response, VerifyHttpSuccess, VerificationAttempt,
    VerificationError,
};

const MAX_RESPONSE_CHARS: usize = 8_000;
const MAX_REQUEST_CHARS: usize = 2_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LlmEndpointVerification {
    is_ai_endpoint: bool,
    confidence: Option<f64>,
    rationale: Option<String>,
}

/// Connectivity check + AI Runtime classification of the response body.
pub async fn verify_target_profile_with_llm(
    profile: &TargetProfile,
    auth_headers: std::collections::HashMap<String, String>,
    llm: &dyn PlannerLlm,
) -> VerificationAttempt {
    let http = match execute_verify_http(profile, auth_headers).await {
        Ok(success) => success,
        Err(attempt) => return attempt,
    };
    validate_http_response_with_llm(profile, &http, llm).await
}

/// AI Runtime classification of a successful connectivity probe.
pub async fn validate_http_response_with_llm(
    profile: &TargetProfile,
    http: &VerifyHttpSuccess,
    llm: &dyn PlannerLlm,
) -> VerificationAttempt {
    let prompt = PromptRegistry::endpoint_verify_user(
        profile.provider.as_str(),
        &profile.framework,
        &profile.full_url(),
        profile.method.as_str(),
        &truncate_for_prompt(&http.request_body, MAX_REQUEST_CHARS),
        http.status_code,
        &truncate_for_prompt(&http.response_text, MAX_RESPONSE_CHARS),
    );

    let raw = match llm.complete(&prompt).await {
        Ok(text) => text,
        Err(err) => {
            warn!(error = %err, "endpoint verify LLM call failed");
            return llm_failure_attempt(&http, VerificationError::LlmValidationFailed(err.to_string()));
        }
    };

    let parsed = match parse_llm_verification(&raw) {
        Ok(value) => value,
        Err(err) => {
            warn!(error = %err, raw = %truncate_for_prompt(&raw, 400), "endpoint verify LLM parse failed");
            return llm_failure_attempt(&http, err);
        }
    };

    if !parsed.is_ai_endpoint {
        let rationale = parsed
            .rationale
            .filter(|text| !text.trim().is_empty())
            .unwrap_or_else(|| "response does not appear to be from an AI system".into());
        return llm_failure_attempt(&http, VerificationError::NotAiEndpoint(rationale));
    }

    let rationale = parsed.rationale.unwrap_or_default();
    let confidence = parsed.confidence.unwrap_or(0.0);
    let message = if rationale.trim().is_empty() {
        format!(
            "Verification succeeded — this is an AI API endpoint (confidence {:.0}%)",
            (confidence * 100.0).clamp(0.0, 100.0)
        )
    } else {
        format!(
            "Verification succeeded — this is an AI API endpoint (confidence {:.0}%): {rationale}",
            (confidence * 100.0).clamp(0.0, 100.0)
        )
    };

    let model = extract_model_from_response(&http.response_text);
    let mut capabilities = profile.default_capabilities.clone();
    if model.is_some() {
        capabilities.supports_conversation = true;
    }

    let preview = preview_body(&http.response_text);
    let mut console = http.console.clone();
    console.success = true;
    console.message = message.clone();
    // Step 2 console should surface AI Runtime classification, not the target HTTP body.
    console.response_preview = None;

    let result = VerificationResult {
        verified: true,
        verified_at: Some(OffsetDateTime::now_utc()),
        provider: profile.provider.as_str().into(),
        model,
        capabilities,
        response_time_ms: http.response_time_ms,
        status_code: http.status_code,
        status: "verified".into(),
        response_preview: preview,
        error_message: None,
    };

    VerificationAttempt {
        console,
        result: Ok(result),
    }
}

fn preview_body(body: &str) -> Option<String> {
    if body.is_empty() {
        return None;
    }
    if body.len() > 8_000 {
        Some(format!("{}…", &body[..8_000]))
    } else {
        Some(body.to_string())
    }
}

fn truncate_for_prompt(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    format!("{}…", &text[..max])
}

fn parse_llm_verification(raw: &str) -> Result<LlmEndpointVerification, VerificationError> {
    let json = extract_json_object(raw)?;
    if let Ok(value) = serde_json::from_str::<LlmEndpointVerification>(&json) {
        return Ok(value);
    }
    parse_llm_verification_lenient(&json).ok_or_else(|| {
        VerificationError::LlmValidationFailed(
            "could not parse isAiEndpoint from LLM response".into(),
        )
    })
}

fn parse_llm_verification_lenient(json: &str) -> Option<LlmEndpointVerification> {
    let is_ai_endpoint = extract_json_bool(json, "isAiEndpoint")
        .or_else(|| extract_json_bool(json, "is_ai_endpoint"))?;
    let confidence = extract_json_number(json, "confidence");
    let rationale = extract_json_string_lenient(json, "rationale");
    Some(LlmEndpointVerification {
        is_ai_endpoint,
        confidence,
        rationale,
    })
}

fn extract_json_bool(json: &str, key: &str) -> Option<bool> {
    let key_quoted = format!("\"{key}\"");
    let mut search_from = 0usize;
    while let Some(rel) = json[search_from..].find(&key_quoted) {
        let after_key = &json[search_from + rel + key_quoted.len()..];
        let after_colon = after_key.trim_start().strip_prefix(':')?;
        let value = after_colon.trim_start();
        if value.starts_with("true") {
            return Some(true);
        }
        if value.starts_with("false") {
            return Some(false);
        }
        search_from += rel + 1;
    }
    None
}

fn extract_json_number(json: &str, key: &str) -> Option<f64> {
    let key_quoted = format!("\"{key}\"");
    let mut search_from = 0usize;
    while let Some(rel) = json[search_from..].find(&key_quoted) {
        let after_key = &json[search_from + rel + key_quoted.len()..];
        let after_colon = after_key.trim_start().strip_prefix(':')?;
        let value = after_colon.trim_start();
        let end = value
            .find([',', '}', '\n'])
            .unwrap_or(value.len());
        let token = value[..end].trim();
        if let Ok(number) = token.parse::<f64>() {
            return Some(number);
        }
        search_from += rel + 1;
    }
    None
}

fn extract_json_string_lenient(json: &str, key: &str) -> Option<String> {
    let key_quoted = format!("\"{key}\"");
    let rel = json.find(&key_quoted)?;
    let after_key = &json[rel + key_quoted.len()..];
    let after_colon = after_key.trim_start().strip_prefix(':')?;
    let mut value = after_colon.trim_start();
    if !value.starts_with('"') {
        return None;
    }
    value = &value[1..];
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '"' {
            break;
        }
        if ch == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('/') => out.push('/'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => break,
            }
            continue;
        }
        out.push(ch);
    }
    if out.is_empty() { None } else { Some(out) }
}

fn extract_json_object(raw: &str) -> Result<String, VerificationError> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Ok(trimmed.to_string());
    }
    let start = trimmed
        .find('{')
        .ok_or_else(|| VerificationError::LlmValidationFailed("LLM response missing JSON".into()))?;
    let mut depth = 0i32;
    for (idx, ch) in trimmed[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(trimmed[start..=start + idx].to_string());
                }
            }
            _ => {}
        }
    }
    Err(VerificationError::LlmValidationFailed(
        "LLM JSON object was incomplete".into(),
    ))
}

fn llm_failure_attempt(
    http: &crate::verification::VerifyHttpSuccess,
    error: VerificationError,
) -> VerificationAttempt {
    let mut console = http.console.clone();
    console.success = false;
    console.message = error.to_string();
    console.response_preview = None;
    VerificationAttempt {
        console,
        result: Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_llm_verification_accepts_camel_case() {
        let raw = r#"{"isAiEndpoint": true, "confidence": 0.92, "rationale": "assistant reply"}"#;
        let parsed = parse_llm_verification(raw).expect("parse");
        assert!(parsed.is_ai_endpoint);
    }

    #[test]
    fn parse_llm_verification_extracts_from_prose() {
        let raw = r#"Here is the result: {"isAiEndpoint": false, "rationale": "validation error"} done."#;
        let parsed = parse_llm_verification(raw).expect("parse");
        assert!(!parsed.is_ai_endpoint);
    }

    #[test]
    fn parse_llm_verification_lenient_when_rationale_has_invalid_escapes() {
        let raw = r#"{
  "isAiEndpoint": true,
  "confidence": 0.91,
  "rationale": "contains bad \escape and "quotes" from upstream"
}"#;
        let parsed = parse_llm_verification(raw).expect("lenient parse");
        assert!(parsed.is_ai_endpoint);
        assert_eq!(parsed.confidence, Some(0.91));
    }

    #[test]
    fn parse_llm_verification_without_rationale() {
        let raw = r#"{"isAiEndpoint": true, "confidence": 0.8}"#;
        let parsed = parse_llm_verification(raw).expect("parse");
        assert!(parsed.is_ai_endpoint);
    }
}
