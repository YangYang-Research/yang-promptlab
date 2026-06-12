use scraper::{Html, Selector};
use tracing::debug;
use url::Url;

use crate::url_policy::normalize_url;

/// Extract crawlable HTTP(S) links from HTML content.
pub fn extract_links(base_url: &str, html: &str) -> Vec<String> {
    let base = match Url::parse(base_url) {
        Ok(url) => url,
        Err(_) => return Vec::new(),
    };

    let document = Html::parse_document(html);
    let mut links = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let selectors = [
        ("a[href]", "href"),
        ("link[href]", "href"),
        ("script[src]", "src"),
        ("iframe[src]", "src"),
        ("form[action]", "action"),
    ];

    for (selector_str, attr) in selectors {
        let Ok(selector) = Selector::parse(selector_str) else {
            continue;
        };

        for element in document.select(&selector) {
            if let Some(raw) = element.value().attr(attr) {
                let raw = raw.trim();
                if raw.is_empty() || raw.starts_with('#') || raw.starts_with("javascript:") {
                    continue;
                }

                if let Some(resolved) = normalize_url(&base, raw) {
                    if resolved.scheme() != "http" && resolved.scheme() != "https" {
                        continue;
                    }
                    let key = resolved.to_string();
                    if seen.insert(key.clone()) {
                        links.push(key);
                    }
                }
            }
        }
    }

    debug!(base = %base_url, count = links.len(), "extracted links");
    links
}

/// An HTML `<form>` discovered while crawling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedForm {
    /// Resolved absolute submission URL.
    pub action: String,
    /// Uppercased HTTP method (defaults to `GET` when unspecified).
    pub method: String,
    /// Named input/select/textarea fields contained in the form.
    pub inputs: Vec<String>,
}

/// Extract HTML forms with resolved action URLs, methods, and field names.
pub fn extract_forms(base_url: &str, html: &str) -> Vec<ExtractedForm> {
    let Ok(base) = Url::parse(base_url) else {
        return Vec::new();
    };

    let document = Html::parse_document(html);
    let Ok(form_selector) = Selector::parse("form") else {
        return Vec::new();
    };
    let field_selector = Selector::parse("input[name], select[name], textarea[name]").ok();

    let mut forms = Vec::new();
    for form in document.select(&form_selector) {
        let method = form
            .value()
            .attr("method")
            .map(|m| m.trim().to_uppercase())
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| "GET".to_string());

        // A form with no/empty action submits back to the current page URL.
        let action = match form.value().attr("action").map(str::trim) {
            Some(raw) if !raw.is_empty() => normalize_url(&base, raw)
                .filter(|u| u.scheme() == "http" || u.scheme() == "https")
                .map(|u| u.to_string())
                .unwrap_or_else(|| base_url.to_string()),
            _ => base_url.to_string(),
        };

        let mut inputs = Vec::new();
        if let Some(field_selector) = &field_selector {
            for field in form.select(field_selector) {
                if let Some(name) = field.value().attr("name") {
                    let name = name.trim();
                    if !name.is_empty() && !inputs.iter().any(|n| n == name) {
                        inputs.push(name.to_string());
                    }
                }
            }
        }

        forms.push(ExtractedForm {
            action,
            method,
            inputs,
        });
    }

    debug!(base = %base_url, count = forms.len(), "extracted forms");
    forms
}

/// Extract resolved JavaScript file URLs referenced via `<script src>`.
pub fn extract_scripts(base_url: &str, html: &str) -> Vec<String> {
    let Ok(base) = Url::parse(base_url) else {
        return Vec::new();
    };

    let document = Html::parse_document(html);
    let Ok(selector) = Selector::parse("script[src]") else {
        return Vec::new();
    };

    let mut scripts = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for element in document.select(&selector) {
        let Some(raw) = element.value().attr("src") else {
            continue;
        };
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        if let Some(resolved) = normalize_url(&base, raw) {
            if resolved.scheme() != "http" && resolved.scheme() != "https" {
                continue;
            }
            let key = resolved.to_string();
            if seen.insert(key.clone()) {
                scripts.push(key);
            }
        }
    }

    debug!(base = %base_url, count = scripts.len(), "extracted scripts");
    scripts
}

/// Extract URL-like strings from inline scripts and JSON blobs (API hints).
pub fn extract_url_hints(content: &str) -> Vec<String> {
    lazy_hints(content)
}

fn lazy_hints(content: &str) -> Vec<String> {
    let mut hints = Vec::new();
    let patterns = [
        r#""(/api[^"\s]*)""#,
        r#""(/v1/[^"\s]+)""#,
        r#""(/graphql[^"\s]*)""#,
        r#""(https?://[^"\s]+)""#,
    ];

    for pattern in patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            for cap in re.captures_iter(content) {
                if let Some(m) = cap.get(1) {
                    hints.push(m.as_str().to_string());
                }
            }
        }
    }

    hints.sort();
    hints.dedup();
    hints
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_anchor_and_form_links() {
        let html = r#"
            <html>
              <body>
                <a href="/about">About</a>
                <a href="https://other.com/x">External</a>
                <form action="/api/submit"></form>
                <script src="/static/app.js"></script>
              </body>
            </html>
        "#;

        let links = extract_links("https://example.com/app/", html);
        assert!(links.contains(&"https://example.com/about".to_string()));
        assert!(links.contains(&"https://example.com/api/submit".to_string()));
        assert!(links.contains(&"https://example.com/static/app.js".to_string()));
        assert!(links.contains(&"https://other.com/x".to_string()));
    }

    #[test]
    fn extracts_api_hints_from_scripts() {
        let js = r#"fetch("/api/v1/chat/completions"); const u = "/graphql";"#;
        let hints = extract_url_hints(js);
        assert!(hints.iter().any(|h| h.contains("/api/v1/chat")));
    }

    #[test]
    fn extracts_forms_with_method_and_inputs() {
        let html = r#"
            <html><body>
              <form action="/login" method="post">
                <input name="username" />
                <input name="password" type="password" />
                <button>Go</button>
              </form>
              <form>
                <input name="q" />
              </form>
            </body></html>
        "#;

        let forms = extract_forms("https://example.com/page", html);
        assert_eq!(forms.len(), 2);

        let login = &forms[0];
        assert_eq!(login.action, "https://example.com/login");
        assert_eq!(login.method, "POST");
        assert_eq!(login.inputs, vec!["username", "password"]);

        // Action-less form submits back to the current page.
        assert_eq!(forms[1].action, "https://example.com/page");
        assert_eq!(forms[1].method, "GET");
        assert_eq!(forms[1].inputs, vec!["q"]);
    }

    #[test]
    fn extracts_javascript_files() {
        let html = r#"
            <html><head>
              <script src="/static/app.js"></script>
              <script src="https://cdn.example.net/lib.js"></script>
              <script>console.log("inline");</script>
            </head></html>
        "#;

        let scripts = extract_scripts("https://example.com/", html);
        assert!(scripts.contains(&"https://example.com/static/app.js".to_string()));
        assert!(scripts.contains(&"https://cdn.example.net/lib.js".to_string()));
        // Inline scripts (no src) are not JavaScript files.
        assert_eq!(scripts.len(), 2);
    }
}
