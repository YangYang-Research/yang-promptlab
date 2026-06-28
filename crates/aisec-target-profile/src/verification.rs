use std::collections::HashMap;
use std::time::Instant;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::StatusCode;
use time::OffsetDateTime;

use crate::prompt::replace_prompt;
use crate::types::{TargetProfile, VerificationResult};

const VERIFY_PROMPT: &str = "Hello";

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

fn extract_model_from_response(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    value
        .pointer("/model")
        .or_else(|| value.pointer("/data/model"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

fn has_ai_response(body: &str, status: StatusCode) -> bool {
    if !status.is_success() {
        return false;
    }
    if body.trim().is_empty() {
        return false;
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        let indicators = [
            "/choices/0/message/content",
            "/content/0/text",
            "/candidates/0/content/parts/0/text",
            "/answer",
            "/output",
            "/result",
            "/response",
            "/message/content",
        ];
        for pointer in indicators {
            if value.pointer(pointer).and_then(|v| v.as_str()).is_some() {
                return true;
            }
        }
        if value.get("error").is_some() {
            return false;
        }
    }
    body.len() > 2
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

/// Execute a real AI request against the target profile for verification.
/// Does NOT use AI Runtime — only validates the target endpoint.
pub async fn verify_target_profile(
    profile: &TargetProfile,
    auth_headers: HashMap<String, String>,
) -> Result<(VerificationResult, VerificationConsoleEntry), VerificationError> {
    if !profile.request_template.contains(&profile.prompt_placeholder) {
        return Err(VerificationError::MissingPromptPlaceholder);
    }

    let url = profile.full_url();
    url::Url::parse(&url).map_err(|e| VerificationError::InvalidUrl(e.to_string()))?;

    let body = replace_prompt(
        &profile.request_template,
        &profile.prompt_placeholder,
        VERIFY_PROMPT,
    );

    let mut headers = profile.headers.clone();
    for (k, v) in auth_headers {
        headers.insert(k, v);
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| VerificationError::ConnectionFailed(e.to_string()))?;

    let header_map = build_header_map(&headers)?;
    let started = Instant::now();

    let response = client
        .post(&url)
        .headers(header_map)
        .body(body.clone())
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                VerificationError::Timeout
            } else {
                VerificationError::ConnectionFailed(e.to_string())
            }
        })?;

    let elapsed = started.elapsed().as_millis() as u64;
    let status = response.status();
    let response_text = response
        .text()
        .await
        .unwrap_or_default();
    let preview = if response_text.len() > 500 {
        Some(format!("{}…", &response_text[..500]))
    } else {
        Some(response_text.clone())
    };

    let success = status.is_success() && has_ai_response(&response_text, status);
    let message = if success {
        "Verification succeeded — target responded with AI content".into()
    } else if status.is_success() {
        VerificationError::NoAiResponse.to_string()
    } else {
        map_status_error(status).to_string()
    };

    let console = VerificationConsoleEntry {
        method: profile.method.as_str().into(),
        url: url.clone(),
        headers: mask_headers(&headers),
        body: body.clone(),
        status_code: status.as_u16(),
        response_time_ms: elapsed,
        response_preview: preview.clone(),
        success,
        message: message.clone(),
    };

    if !status.is_success() {
        return Err(map_status_error(status));
    }
    if !has_ai_response(&response_text, status) {
        return Err(VerificationError::NoAiResponse);
    }

    let model = extract_model_from_response(&response_text);
    let mut capabilities = profile.default_capabilities.clone();
    if model.is_some() {
        capabilities.supports_conversation = true;
    }

    let result = VerificationResult {
        verified: true,
        verified_at: Some(OffsetDateTime::now_utc()),
        provider: profile.provider.as_str().into(),
        model,
        capabilities,
        response_time_ms: elapsed,
        status_code: status.as_u16(),
        status: "verified".into(),
        response_preview: preview,
        error_message: None,
    };

    Ok((result, console))
}
