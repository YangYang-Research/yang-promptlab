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
}
