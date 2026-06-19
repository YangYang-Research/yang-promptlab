//! Default HTTP method inference from endpoint paths.

const POST_KEYWORDS: &[&str] = &[
    "chat",
    "completion",
    "generate",
    "predict",
    "invoke",
    "messages",
    "agent",
    "workflow",
    "ask",
    "query",
    "prompt",
];

const GET_KEYWORDS: &[&str] = &[
    "health",
    "status",
    "metrics",
    "version",
    "swagger",
    "openapi",
];

/// Infer a default HTTP method from a URL or path segment.
pub fn default_http_method_for_path(path_or_url: &str) -> &'static str {
    let lower = path_or_url.to_ascii_lowercase();
    for keyword in POST_KEYWORDS {
        if lower.contains(keyword) {
            return "POST";
        }
    }
    for keyword in GET_KEYWORDS {
        if lower.contains(keyword) {
            return "GET";
        }
    }
    "GET"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_endpoints_default_post() {
        assert_eq!(default_http_method_for_path("/v1/chat/completions"), "POST");
        assert_eq!(default_http_method_for_path("/api/generate"), "POST");
    }

    #[test]
    fn health_endpoints_default_get() {
        assert_eq!(default_http_method_for_path("/health"), "GET");
        assert_eq!(default_http_method_for_path("/metrics"), "GET");
    }

    #[test]
    fn unknown_defaults_get() {
        assert_eq!(default_http_method_for_path("/api/users"), "GET");
    }
}
