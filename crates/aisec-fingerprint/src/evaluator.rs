use regex::Regex;
use url::Url;

use crate::rules::{FingerprintRule, RuleMatcher};
use crate::types::FingerprintInput;

pub fn evaluate_rule(rule: &FingerprintRule, input: &FingerprintInput) -> bool {
    match &rule.matcher {
        RuleMatcher::HostContains(needle) => host(input)
            .is_some_and(|h| h.to_lowercase().contains(&needle.to_lowercase())),
        RuleMatcher::HostRegex(pattern) => host(input)
            .is_some_and(|h| regex_cached(pattern).is_match(&h)),
        RuleMatcher::UrlContains(needle) => input.url.to_lowercase().contains(&needle.to_lowercase()),
        RuleMatcher::PathRegex(pattern) => path(input)
            .is_some_and(|p| regex_cached(pattern).is_match(&p)),
        RuleMatcher::HeaderPresent(name) => header(input, name).is_some(),
        RuleMatcher::HeaderContains { name, value } => header(input, name)
            .is_some_and(|v| v.to_lowercase().contains(&value.to_lowercase())),
        RuleMatcher::BodyContains(needle) => input
            .body
            .as_deref()
            .is_some_and(|b| b.contains(needle)),
        RuleMatcher::BodyJsonField { pointer, equals } => json_field(input, pointer, *equals),
        RuleMatcher::BodyJsonArrayContains { pointer, field, value } => {
            json_array_contains(input, pointer, field, value)
        }
        RuleMatcher::StatusIn(codes) => input.status.is_some_and(|s| codes.contains(&s)),
    }
}

fn host(input: &FingerprintInput) -> Option<String> {
    Url::parse(&input.url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
}

fn path(input: &FingerprintInput) -> Option<String> {
    Url::parse(&input.url).ok().map(|u| {
        let mut p = u.path().to_string();
        if let Some(q) = u.query() {
            p.push('?');
            p.push_str(q);
        }
        p
    })
}

fn header(input: &FingerprintInput, name: &str) -> Option<String> {
    let lower = name.to_lowercase();
    input
        .headers
        .iter()
        .find(|(k, _)| k.to_lowercase() == lower)
        .map(|(_, v)| v.clone())
}

fn body_json(input: &FingerprintInput) -> Option<serde_json::Value> {
    let body = input.body.as_deref()?;
    serde_json::from_str(body).ok()
}

fn json_field(input: &FingerprintInput, pointer: &str, equals: Option<&str>) -> bool {
    let Some(value) = body_json(input) else {
        return false;
    };
    let Some(found) = value.pointer(pointer) else {
        return false;
    };
    match equals {
        Some(expected) => found.as_str() == Some(expected),
        None => !found.is_null(),
    }
}

fn json_array_contains(input: &FingerprintInput, pointer: &str, field: &str, value: &str) -> bool {
    let Some(root) = body_json(input) else {
        return false;
    };
    let Some(arr) = root.pointer(pointer).and_then(|v| v.as_array()) else {
        return false;
    };
    arr.iter().any(|item| {
        item.get(field)
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.to_lowercase().contains(&value.to_lowercase()))
    })
}

fn regex_cached(pattern: &str) -> &'static Regex {
    regex_cached_slow(pattern)
}

fn regex_cached_slow(pattern: &str) -> &'static Regex {
    // leak compiled regex for static patterns
    Box::leak(Box::new(Regex::new(pattern).expect("invalid rule regex")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::rule_catalog;
    use crate::types::AiProvider;
    use std::collections::HashMap;

    #[test]
    fn openai_host_rule_matches() {
        let rule = rule_catalog()
            .into_iter()
            .find(|r| r.id == "openai.host")
            .unwrap();
        let input = FingerprintInput {
            url: "https://api.openai.com/v1/models".into(),
            method: None,
            status: Some(200),
            headers: HashMap::new(),
            body: None,
        };
        assert!(evaluate_rule(&rule, &input));
    }

    #[test]
    fn anthropic_header_rule_matches() {
        let rule = rule_catalog()
            .into_iter()
            .find(|r| r.id == "anthropic.header.version")
            .unwrap();
        assert_eq!(rule.provider, AiProvider::Anthropic);
        let mut headers = HashMap::new();
        headers.insert("anthropic-version".into(), "2023-06-01".into());
        let input = FingerprintInput {
            url: "https://example.com/v1/messages".into(),
            method: Some("POST".into()),
            status: Some(401),
            headers,
            body: None,
        };
        assert!(evaluate_rule(&rule, &input));
    }
}
