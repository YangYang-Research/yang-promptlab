use std::collections::HashMap;
use std::time::Instant;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Method, StatusCode};
use time::OffsetDateTime;

use crate::prompt::replace_prompt;
use crate::types::{HttpMethod, TargetProfile, VerificationResult};

const VERIFY_PROMPT: &str = "Hello";
pub(crate) const VERIFY_PROBE: &str = VERIFY_PROMPT;

#[derive(Debug, Clone, thiserror::Error)]
pub enum VerificationError {
    #[error("request template must contain the prompt placeholder")]
    MissingPromptPlaceholder,
    #[error("invalid target URL: {0}")]
    InvalidUrl(String),
    #[error("401 Unauthorized — invalid credentials")]
    Unauthorized,
    #[error("403 Forbidden — permission denied")]
    Forbidden,
    #[error("404 Not Found — wrong endpoint path")]
    NotFound,
    #[error("422 Unprocessable Entity — invalid payload")]
    UnprocessableEntity,
    #[error("429 Too Many Requests — rate limited")]
    RateLimited,
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("request timed out")]
    Timeout,
    #[error("HTTP {0} — verification failed")]
    HttpError(u16),
    #[error("response did not contain AI content")]
    NoAiResponse,
    #[error("Yazg rejected endpoint: {0}")]
    NotAiEndpoint(String),
    #[error("Yazg validation failed: {0}")]
    LlmValidationFailed(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationConsoleEntry {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub status_code: u16,
    pub response_time_ms: u64,
    pub response_preview: Option<String>,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct VerificationAttempt {
    pub console: VerificationConsoleEntry,
    pub result: Result<VerificationResult, VerificationError>,
}

fn mask_sensitive(value: &str) -> String {
    if value.len() <= 8 {
        return "***".into();
    }
    format!("{}…{}", &value[..4], &value[value.len() - 2..])
}

fn mask_headers(headers: &HashMap<String, String>) -> HashMap<String, String> {
    headers
        .iter()
        .map(|(k, v)| {
            let lower = k.to_ascii_lowercase();
            let masked = if lower.contains("authorization")
                || lower.contains("api-key")
                || lower.contains("x-api-key")
                || lower.contains("token")
                || lower.contains("cookie")
            {
                mask_sensitive(v)
            } else {
                v.clone()
            };
            (k.clone(), masked)
        })
        .collect()
}

fn is_likely_auth_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    const NON_AUTH: &[&str] = &[
        "content-type",
        "accept",
        "user-agent",
        "accept-encoding",
        "accept-language",
        "cache-control",
        "connection",
        "host",
        "origin",
        "referer",
    ];
    if NON_AUTH.contains(&lower.as_str()) {
        return false;
    }
    if lower == "authorization" {
        return true;
    }
    const API_KEY_HEADERS: &[&str] = &[
        "x-api-key",
        "api-key",
        "anthropic-api-key",
        "x-goog-api-key",
        "openai-api-key",
    ];
    if API_KEY_HEADERS.contains(&lower.as_str()) {
        return true;
    }
    if lower.ends_with("-key") {
        return true;
    }
    const X_NON_AUTH: &[&str] = &[
        "x-request-id",
        "x-correlation-id",
        "x-trace-id",
        "x-forwarded-for",
        "x-forwarded-proto",
        "x-forwarded-host",
        "x-real-ip",
        "x-frame-options",
        "x-content-type-options",
    ];
    if lower.starts_with("x-") && !X_NON_AUTH.contains(&lower.as_str()) {
        return true;
    }
    lower.contains("api-token")
        || lower.ends_with("-token")
        || lower.contains("api_key")
        || lower.contains("api-key")
        || lower.contains("credential")
        || lower.contains("secret")
        || (lower.contains("auth") && lower != "authority")
}

fn merge_profile_and_auth_headers(
    profile_headers: &HashMap<String, String>,
    auth_headers: HashMap<String, String>,
) -> HashMap<String, String> {
    if auth_headers.is_empty() {
        return profile_headers.clone();
    }

    let mut headers: HashMap<String, String> = profile_headers
        .iter()
        .filter(|(name, _)| !is_likely_auth_header(name))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    for (key, value) in auth_headers {
        headers.insert(key, value);
    }

    headers
}

fn build_header_map(headers: &HashMap<String, String>) -> Result<HeaderMap, VerificationError> {
    let mut map = HeaderMap::new();
    for (key, value) in headers {
        let name = HeaderName::from_bytes(key.as_bytes())
            .map_err(|e| VerificationError::ConnectionFailed(e.to_string()))?;
        let val = HeaderValue::from_str(value)
            .map_err(|e| VerificationError::ConnectionFailed(e.to_string()))?;
        map.insert(name, val);
    }
    Ok(map)
}

fn preflight_console(
    profile: &TargetProfile,
    headers: &HashMap<String, String>,
    body: &str,
    message: impl Into<String>,
) -> VerificationConsoleEntry {
    VerificationConsoleEntry {
        method: profile.method.as_str().into(),
        url: profile.full_url(),
        headers: mask_headers(headers),
        body: body.to_string(),
        status_code: 0,
        response_time_ms: 0,
        response_preview: None,
        success: false,
        message: message.into(),
    }
}

fn attempt_failure(
    profile: &TargetProfile,
    headers: &HashMap<String, String>,
    body: &str,
    error: VerificationError,
) -> VerificationAttempt {
    VerificationAttempt {
        console: preflight_console(profile, headers, body, error.to_string()),
        result: Err(error),
    }
}

fn reqwest_method(method: HttpMethod) -> Method {
    match method {
        HttpMethod::Get => Method::GET,
        HttpMethod::Post => Method::POST,
        HttpMethod::Put => Method::PUT,
        HttpMethod::Patch => Method::PATCH,
        HttpMethod::Delete => Method::DELETE,
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyHttpSuccess {
    pub console: VerificationConsoleEntry,
    pub response_text: String,
    pub request_body: String,
    pub response_time_ms: u64,
    pub status_code: u16,
}

pub(crate) fn extract_model_from_response(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    value
        .pointer("/model")
        .or_else(|| value.pointer("/model_name"))
        .or_else(|| value.pointer("/data/model"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

fn json_has_text_content(value: &serde_json::Value) -> bool {
    let indicators = [
        "/choices/0/message/content",
        "/content/0/text",
        "/candidates/0/content/parts/0/text",
        "/answer",
        "/output",
        "/result",
        "/response",
        "/message/content",
        "/data/choices/0/message/content",
        "/data/message/content",
    ];
    for pointer in indicators {
        if value
            .pointer(pointer)
            .and_then(|v| v.as_str())
            .is_some_and(|text| !text.trim().is_empty())
        {
            return true;
        }
    }

    if let Some(messages) = value.get("messages").and_then(|v| v.as_array()) {
        for message in messages {
            if message
                .get("content")
                .and_then(|v| v.as_str())
                .is_some_and(|text| !text.trim().is_empty())
            {
                return true;
            }
            if message
                .get("content")
                .and_then(|v| v.as_array())
                .is_some_and(|parts| {
                    parts.iter().any(|part| {
                        part.get("text")
                            .and_then(|v| v.as_str())
                            .is_some_and(|text| !text.trim().is_empty())
                    })
                })
            {
                return true;
            }
        }
    }

    false
}

fn has_ai_response(body: &str, status: StatusCode) -> bool {
    if !status.is_success() {
        return false;
    }
    if body.trim().is_empty() {
        return false;
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        if value.get("error").is_some() || value.get("detail").is_some() {
            return false;
        }
        if json_has_text_content(&value) {
            return true;
        }
        // OpenAI-style streaming=false may return id + choices without nested content in edge cases.
        if value.get("choices").and_then(|v| v.as_array()).is_some_and(|c| !c.is_empty()) {
            return true;
        }
    }
    body.trim().len() > 2
}

fn map_status_error(status: StatusCode) -> VerificationError {
    match status.as_u16() {
        401 => VerificationError::Unauthorized,
        403 => VerificationError::Forbidden,
        404 => VerificationError::NotFound,
        422 => VerificationError::UnprocessableEntity,
        429 => VerificationError::RateLimited,
        code => VerificationError::HttpError(code),
    }
}

/// Execute the real HTTP verify probe (connectivity only).
pub async fn execute_verify_http(
    profile: &TargetProfile,
    auth_headers: HashMap<String, String>,
) -> Result<VerifyHttpSuccess, VerificationAttempt> {
    if !profile.request_template.contains(&profile.prompt_placeholder) {
        return Err(attempt_failure(
            profile,
            &profile.headers,
            "",
            VerificationError::MissingPromptPlaceholder,
        ));
    }

    let url = profile.full_url();
    if url::Url::parse(&url).is_err() {
        return Err(attempt_failure(
            profile,
            &profile.headers,
            "",
            VerificationError::InvalidUrl(url.clone()),
        ));
    }

    let body = replace_prompt(
        &profile.request_template,
        &profile.prompt_placeholder,
        VERIFY_PROMPT,
    );

    let mut headers = merge_profile_and_auth_headers(&profile.headers, auth_headers);

    if profile.method != HttpMethod::Get {
        let has_content_type = headers.keys().any(|name| name.eq_ignore_ascii_case("content-type"));
        let body_trimmed = body.trim();
        if !has_content_type
            && (body_trimmed.starts_with('{') || body_trimmed.starts_with('['))
        {
            headers.insert("Content-Type".into(), "application/json".into());
        }
    }

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            return Err(attempt_failure(
                profile,
                &headers,
                &body,
                VerificationError::ConnectionFailed(e.to_string()),
            ));
        }
    };

    let header_map = match build_header_map(&headers) {
        Ok(map) => map,
        Err(error) => return Err(attempt_failure(profile, &headers, &body, error)),
    };

    let started = Instant::now();
    let mut request = client
        .request(reqwest_method(profile.method), &url)
        .headers(header_map);

    if profile.method != HttpMethod::Get {
        request = request.body(body.clone());
    }

    let response = match request.send().await {
        Ok(response) => response,
        Err(e) => {
            let error = if e.is_timeout() {
                VerificationError::Timeout
            } else {
                VerificationError::ConnectionFailed(e.to_string())
            };
            return Err(attempt_failure(profile, &headers, &body, error));
        }
    };

    let elapsed = started.elapsed().as_millis() as u64;
    let status = response.status();
    let response_text = response.text().await.unwrap_or_default();
    let preview = if response_text.len() > 8000 {
        Some(format!("{}…", &response_text[..8000]))
    } else if response_text.is_empty() {
        None
    } else {
        Some(response_text.clone())
    };

    let console = VerificationConsoleEntry {
        method: profile.method.as_str().into(),
        url: url.clone(),
        headers: mask_headers(&headers),
        body: body.clone(),
        status_code: status.as_u16(),
        response_time_ms: elapsed,
        response_preview: preview,
        success: false,
        message: String::new(),
    };

    if !status.is_success() {
        let message = map_status_error(status).to_string();
        return Err(VerificationAttempt {
            console: VerificationConsoleEntry {
                message,
                ..console
            },
            result: Err(map_status_error(status)),
        });
    }

    if response_text.trim().is_empty() {
        let message = VerificationError::NoAiResponse.to_string();
        return Err(VerificationAttempt {
            console: VerificationConsoleEntry {
                message: message.clone(),
                ..console
            },
            result: Err(VerificationError::NoAiResponse),
        });
    }

    Ok(VerifyHttpSuccess {
        console: VerificationConsoleEntry {
            message: "Authentication and connectivity verified".into(),
            ..console
        },
        response_text,
        request_body: body,
        response_time_ms: elapsed,
        status_code: status.as_u16(),
    })
}

/// Heuristic-only verification (no Yazg). Used by integration tests.
pub async fn verify_target_profile(
    profile: &TargetProfile,
    auth_headers: HashMap<String, String>,
) -> VerificationAttempt {
    let http = match execute_verify_http(profile, auth_headers).await {
        Ok(success) => success,
        Err(attempt) => return attempt,
    };

    let status = StatusCode::from_u16(http.status_code).unwrap_or(StatusCode::OK);
    let success = has_ai_response(&http.response_text, status);
    let message = if success {
        "Verification succeeded — target responded with AI content".into()
    } else {
        VerificationError::NoAiResponse.to_string()
    };

    let mut console = http.console;
    console.success = success;
    console.message = message;

    if !success {
        return VerificationAttempt {
            console,
            result: Err(VerificationError::NoAiResponse),
        };
    }

    let model = extract_model_from_response(&http.response_text);
    let mut capabilities = profile.default_capabilities.clone();
    if model.is_some() {
        capabilities.supports_conversation = true;
    }

    let preview = if http.response_text.len() > 8000 {
        Some(format!("{}…", &http.response_text[..8000]))
    } else {
        Some(http.response_text.clone())
    };

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

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    #[test]
    fn accepts_plain_text_success_response() {
        assert!(has_ai_response(
            "Hello! I'm Yang Code Review, ready to help.",
            StatusCode::OK,
        ));
    }

    #[test]
    fn rejects_fastapi_validation_error_json() {
        let body = r#"{"detail":[{"type":"missing","loc":["body","model_name"]}]}"#;
        assert!(!has_ai_response(body, StatusCode::OK));
    }

    #[test]
    fn rejects_error_object_on_success_status() {
        assert!(!has_ai_response(
            r#"{"error":"Internal server error"}"#,
            StatusCode::OK,
        ));
    }

    #[test]
    fn merge_profile_and_auth_headers_replaces_credential_headers() {
        let mut profile_headers = HashMap::new();
        profile_headers.insert("Authorization".into(), "Bearer old".into());
        profile_headers.insert("Content-Type".into(), "application/json".into());

        let mut auth_headers = HashMap::new();
        auth_headers.insert("x-api-key".into(), "secret".into());

        let merged = merge_profile_and_auth_headers(&profile_headers, auth_headers);
        assert!(!merged.contains_key("Authorization"));
        assert_eq!(merged.get("x-api-key").map(String::as_str), Some("secret"));
        assert_eq!(
            merged.get("Content-Type").map(String::as_str),
            Some("application/json")
        );
    }
}
