//! Map target `descriptor_json.auth` onto [`AttackTarget`] HTTP headers.
//!
//! Credential auth (`basic`, `api_key`, `jwt`) follows `promptlab-auth` header semantics.
//! Playwright-backed auth applies persisted session tokens when present.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;

use crate::types::AttackTarget;

#[derive(Debug, Clone, Deserialize)]
struct DescriptorAuth {
    kind: String,
    #[serde(default)]
    config: Option<serde_json::Value>,
    #[serde(default)]
    tokens: Option<Vec<DescriptorToken>>,
    // Legacy flat fields (pre-auth redesign)
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    header: Option<String>,
    #[serde(default)]
    value: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DescriptorToken {
    #[serde(default)]
    header_name: Option<String>,
    value: String,
}

#[derive(Debug, Clone, Deserialize)]
struct BasicConfig {
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiKeyConfig {
    key: String,
    header_name: String,
    #[serde(default)]
    prefix: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct JwtConfig {
    token: String,
    #[serde(default)]
    header_name: Option<String>,
    #[serde(default)]
    prefix: Option<String>,
}

/// Apply auth from a target descriptor JSON string onto an attack target.
pub fn apply_descriptor_auth(target: AttackTarget, descriptor_json: &str) -> AttackTarget {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(descriptor_json) else {
        return target;
    };
    apply_descriptor_auth_value(target, &value)
}

/// Apply auth from a parsed target descriptor value onto an attack target.
pub fn apply_descriptor_auth_value(
    target: AttackTarget,
    descriptor: &serde_json::Value,
) -> AttackTarget {
    let Some(auth_value) = descriptor.get("auth") else {
        return target;
    };
    let Ok(auth) = serde_json::from_value::<DescriptorAuth>(auth_value.clone()) else {
        return target;
    };
    apply_auth_config(target, &auth)
}

fn apply_auth_config(mut target: AttackTarget, auth: &DescriptorAuth) -> AttackTarget {
    if let Some(tokens) = &auth.tokens {
        for token in tokens {
            if let Some(header_name) = token.header_name.as_deref() {
                if !header_name.trim().is_empty() && !token.value.trim().is_empty() {
                    target = target.with_header(header_name, token.value.trim());
                }
            }
        }
    }

    match auth.kind.as_str() {
        "basic" => target = apply_basic_auth(target, auth),
        "api_key" => target = apply_api_key_auth(target, auth),
        "jwt" => target = apply_jwt_auth(target, auth),
        // Playwright methods apply headers only when session tokens are attached later (B9).
        _ => {}
    }

    target
}

fn apply_basic_auth(target: AttackTarget, auth: &DescriptorAuth) -> AttackTarget {
    let (username, password) = if let Some(config) = &auth.config {
        if let Ok(parsed) = serde_json::from_value::<BasicConfig>(config.clone()) {
            (
                parsed.username.unwrap_or_default(),
                parsed.password.unwrap_or_default(),
            )
        } else {
            (String::new(), String::new())
        }
    } else {
        (
            auth.username.clone().unwrap_or_default(),
            auth.password.clone().unwrap_or_default(),
        )
    };

    let username = username.trim();
    if username.is_empty() {
        return target;
    }

    let encoded = STANDARD.encode(format!("{username}:{password}"));
    target.with_header("Authorization", format!("Basic {encoded}"))
}

fn format_credential_with_prefix(prefix: Option<&str>, credential: &str) -> String {
    let trimmed = credential.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let Some(raw_prefix) = prefix.filter(|p| !p.is_empty()) else {
        return trimmed.to_string();
    };

    let scheme = raw_prefix.trim();
    if trimmed.starts_with(raw_prefix)
        || (!scheme.is_empty()
            && trimmed
                .to_ascii_lowercase()
                .starts_with(&format!("{} ", scheme.to_ascii_lowercase())))
    {
        return trimmed.to_string();
    }

    let normalized = if scheme.eq_ignore_ascii_case("basic")
        || scheme.eq_ignore_ascii_case("bearer")
        || scheme.eq_ignore_ascii_case("token")
    {
        format!("{scheme} ")
    } else {
        raw_prefix.to_string()
    };
    format!("{normalized}{trimmed}")
}

fn apply_api_key_auth(target: AttackTarget, auth: &DescriptorAuth) -> AttackTarget {
    if let Some(config) = &auth.config {
        if let Ok(parsed) = serde_json::from_value::<ApiKeyConfig>(config.clone()) {
            if !parsed.key.trim().is_empty() {
                let value = format_credential_with_prefix(parsed.prefix.as_deref(), &parsed.key);
                return target.with_header(parsed.header_name.trim(), value);
            }
        }
    }

    // Legacy `{ header, value }`
    let header = auth
        .header
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("Authorization");
    let value = auth.value.as_deref().unwrap_or("").trim();
    if value.is_empty() {
        return target;
    }
    target.with_header(header, value)
}

fn apply_jwt_auth(target: AttackTarget, auth: &DescriptorAuth) -> AttackTarget {
    let Some(config) = &auth.config else {
        return target;
    };
    let Ok(parsed) = serde_json::from_value::<JwtConfig>(config.clone()) else {
        return target;
    };
    if parsed.token.trim().is_empty() {
        return target;
    }

    let header = parsed
        .header_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("Authorization");
    let value = format_credential_with_prefix(parsed.prefix.as_deref(), &parsed.token);
    target.with_header(header, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn auth_headers(descriptor: serde_json::Value) -> HashMap<String, String> {
        let target = apply_descriptor_auth_value(
            AttackTarget::llm_api("https://api.example.com/v1/chat"),
            &descriptor,
        );
        target.headers
    }

    #[test]
    fn none_auth_adds_no_headers() {
        let headers = auth_headers(serde_json::json!({
            "url": "https://api.example.com",
            "auth": { "kind": "none", "engine": "none" }
        }));
        assert!(headers.is_empty());
    }

    #[test]
    fn basic_auth_sets_authorization_header() {
        let headers = auth_headers(serde_json::json!({
            "url": "https://api.example.com",
            "auth": {
                "kind": "basic",
                "engine": "auth_engine",
                "config": { "username": "alice", "password": "secret" }
            }
        }));
        assert_eq!(
            headers.get("Authorization").map(String::as_str),
            Some("Basic YWxpY2U6c2VjcmV0")
        );
    }

    #[test]
    fn api_key_auth_uses_auth_engine_config() {
        let headers = auth_headers(serde_json::json!({
            "url": "https://api.example.com",
            "auth": {
                "kind": "api_key",
                "engine": "auth_engine",
                "config": {
                    "type": "api_key",
                    "key": "sk-test",
                    "header_name": "X-API-Key",
                    "prefix": null
                }
            }
        }));
        assert_eq!(headers.get("X-API-Key").map(String::as_str), Some("sk-test"));
    }

    #[test]
    fn api_key_prefix_is_applied() {
        let headers = auth_headers(serde_json::json!({
            "url": "https://api.example.com",
            "auth": {
                "kind": "api_key",
                "engine": "auth_engine",
                "config": {
                    "type": "api_key",
                    "key": "sk-test",
                    "header_name": "Authorization",
                    "prefix": "Bearer "
                }
            }
        }));
        assert_eq!(
            headers.get("Authorization").map(String::as_str),
            Some("Bearer sk-test")
        );
    }

    #[test]
    fn jwt_auth_uses_bearer_prefix() {
        let headers = auth_headers(serde_json::json!({
            "url": "https://api.example.com",
            "auth": {
                "kind": "jwt",
                "engine": "auth_engine",
                "config": {
                    "type": "jwt",
                    "token": "eyJ.test",
                    "header_name": "Authorization",
                    "prefix": "Bearer "
                }
            }
        }));
        assert_eq!(
            headers.get("Authorization").map(String::as_str),
            Some("Bearer eyJ.test")
        );
    }

    #[test]
    fn legacy_api_key_header_value_still_works() {
        let headers = auth_headers(serde_json::json!({
            "url": "https://api.example.com",
            "auth": { "kind": "api_key", "header": "X-API-Key", "value": "legacy" }
        }));
        assert_eq!(headers.get("X-API-Key").map(String::as_str), Some("legacy"));
    }

    #[test]
    fn playwright_session_tokens_apply_headers() {
        let headers = auth_headers(serde_json::json!({
            "url": "https://api.example.com",
            "auth": {
                "kind": "username_password",
                "engine": "playwright",
                "tokens": [
                    { "header_name": "Authorization", "value": "Bearer session-token" }
                ]
            }
        }));
        assert_eq!(
            headers.get("Authorization").map(String::as_str),
            Some("Bearer session-token")
        );
    }
}
