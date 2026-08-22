use crate::models::{AttackRequest, NormalizedResponse};

const REDACTED: &str = "[REDACTED]";

/// Strip request secrets from persisted harness output.
pub fn redact_response(request: &AttackRequest, response: &mut NormalizedResponse) {
    let mut secrets: Vec<String> = Vec::new();
    push_secret(&mut secrets, request.auth.bearer_token.as_deref());
    push_secret(&mut secrets, request.auth.basic_password.as_deref());
    push_secret(&mut secrets, request.auth.api_key.as_deref());
    push_secret(&mut secrets, request.auth.query_key_value.as_deref());
    push_secret(&mut secrets, request.auth.aws_secret_access_key.as_deref());
    push_secret(&mut secrets, request.auth.aws_session_token.as_deref());
    push_secret(&mut secrets, request.auth.cookie_header.as_deref());
    for value in request.auth.headers.values() {
        push_secret(&mut secrets, Some(value));
    }
    for (key, value) in &request.headers {
        if is_sensitive_header(key) {
            push_secret(&mut secrets, Some(value));
        }
    }

    for secret in secrets {
        if secret.len() < 8 {
            continue;
        }
        if !response.raw_response.is_empty() {
            response.raw_response = response.raw_response.replace(&secret, REDACTED);
        }
        if !response.content.is_empty() {
            response.content = response.content.replace(&secret, REDACTED);
        }
    }
}

fn push_secret(out: &mut Vec<String>, value: Option<&str>) {
    if let Some(text) = value.map(str::trim).filter(|s| s.len() >= 8) {
        out.push(text.to_string());
    }
}

fn is_sensitive_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("authorization")
        || lower.contains("api-key")
        || lower.contains("apikey")
        || lower.contains("token")
        || lower.contains("secret")
        || lower.contains("password")
        || lower == "cookie"
        || lower == "x-api-key"
        || lower == "x-goog-api-key"
}
