//! OpenAPI / Swagger specification analysis for AI provider detection.
//!
//! Discovery often yields an OpenAPI document rather than a live response. This
//! module turns a spec's declared servers and operations into synthetic
//! [`FingerprintInput`]s so the standard request-pattern rules (host/path) can
//! classify the provider, returning the same confidence-scored results.

use serde_json::Value;

use crate::types::FingerprintInput;

/// Base used for relative specs (no `servers`) so paths still parse as URLs.
/// Chosen to never match a real provider host.
const SYNTHETIC_BASE: &str = "http://openapi.spec.local";

/// Build fingerprintable HTTP observations from an OpenAPI/Swagger spec by
/// enumerating every (server, path, method) combination.
pub fn inputs_from_openapi(spec: &Value) -> Vec<FingerprintInput> {
    let servers = server_urls(spec);
    let operations = operations(spec);

    let bases: Vec<String> = if servers.is_empty() {
        vec![SYNTHETIC_BASE.to_string()]
    } else {
        servers
    };

    let mut out = Vec::new();

    if operations.is_empty() {
        // No paths declared: still emit server-only inputs for host detection.
        for base in &bases {
            if let Some(url) = normalize_base(base) {
                out.push(FingerprintInput {
                    url,
                    ..Default::default()
                });
            }
        }
        return out;
    }

    for base in &bases {
        for (path, method) in &operations {
            if let Some(url) = join(base, path) {
                out.push(FingerprintInput {
                    url,
                    method: method.clone(),
                    ..Default::default()
                });
            }
        }
    }
    out
}

/// Extract server base URLs from an OpenAPI 3 (`servers[].url`) or Swagger 2
/// (`host` + `basePath` + `schemes`) document.
fn server_urls(spec: &Value) -> Vec<String> {
    let mut urls = Vec::new();

    if let Some(servers) = spec.get("servers").and_then(|v| v.as_array()) {
        for s in servers {
            if let Some(u) = s.get("url").and_then(|v| v.as_str()) {
                if !u.is_empty() {
                    urls.push(u.to_string());
                }
            }
        }
    }

    if urls.is_empty() {
        if let Some(host) = spec.get("host").and_then(|v| v.as_str()) {
            let scheme = spec
                .get("schemes")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .unwrap_or("https");
            let base_path = spec.get("basePath").and_then(|v| v.as_str()).unwrap_or("");
            urls.push(format!("{scheme}://{host}{base_path}"));
        }
    }

    urls
}

/// Extract `(path, first-declared-method)` pairs from the spec's `paths` object.
fn operations(spec: &Value) -> Vec<(String, Option<String>)> {
    let mut ops = Vec::new();
    let Some(paths) = spec.get("paths").and_then(|v| v.as_object()) else {
        return ops;
    };

    for (path, item) in paths {
        let method = item.as_object().and_then(|m| {
            ["post", "get", "put", "patch", "delete"]
                .iter()
                .find(|verb| m.contains_key(**verb))
                .map(|verb| verb.to_uppercase())
        });
        ops.push((path.clone(), method));
    }
    ops
}

fn normalize_base(base: &str) -> Option<String> {
    let base = base.trim();
    if base.is_empty() {
        return Some(SYNTHETIC_BASE.to_string());
    }
    if base.starts_with("http://") || base.starts_with("https://") {
        Some(base.to_string())
    } else if let Some(rest) = base.strip_prefix("//") {
        Some(format!("https://{rest}"))
    } else if base.starts_with('/') {
        Some(format!("{SYNTHETIC_BASE}{base}"))
    } else if base.contains('.') {
        // Looks like a bare host (Swagger 2 style already handled, but be safe).
        Some(format!("https://{base}"))
    } else {
        Some(format!("{SYNTHETIC_BASE}/{base}"))
    }
}

fn join(base: &str, path: &str) -> Option<String> {
    let base = normalize_base(base)?;
    let base = base.trim_end_matches('/');
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    Some(format!("{base}{path}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_inputs_from_servers_and_paths() {
        let spec = json!({
            "openapi": "3.0.0",
            "servers": [{ "url": "https://api.openai.com/v1" }],
            "paths": {
                "/chat/completions": { "post": {} },
                "/models": { "get": {} }
            }
        });
        let inputs = inputs_from_openapi(&spec);
        assert_eq!(inputs.len(), 2);
        assert!(inputs
            .iter()
            .any(|i| i.url == "https://api.openai.com/v1/chat/completions"
                && i.method.as_deref() == Some("POST")));
        assert!(inputs
            .iter()
            .any(|i| i.url == "https://api.openai.com/v1/models"
                && i.method.as_deref() == Some("GET")));
    }

    #[test]
    fn handles_swagger_2_host_and_basepath() {
        let spec = json!({
            "swagger": "2.0",
            "host": "api.anthropic.com",
            "basePath": "/v1",
            "schemes": ["https"],
            "paths": { "/messages": { "post": {} } }
        });
        let inputs = inputs_from_openapi(&spec);
        assert!(inputs
            .iter()
            .any(|i| i.url == "https://api.anthropic.com/v1/messages"));
    }

    #[test]
    fn relative_spec_uses_synthetic_base() {
        let spec = json!({
            "openapi": "3.0.0",
            "paths": { "/v1/chat/completions": { "post": {} } }
        });
        let inputs = inputs_from_openapi(&spec);
        assert_eq!(inputs.len(), 1);
        assert!(inputs[0].url.ends_with("/v1/chat/completions"));
        assert!(inputs[0].url.starts_with("http://"));
    }
}
