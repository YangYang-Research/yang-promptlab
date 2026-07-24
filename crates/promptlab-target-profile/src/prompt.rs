pub const PROMPT_PLACEHOLDER: &str = "{{PROMPT}}";

/// Replace only the prompt placeholder in the request template body.
/// Does not modify any other JSON fields.
pub fn replace_prompt(template: &str, placeholder: &str, prompt: &str) -> String {
    let needle = if placeholder.is_empty() {
        PROMPT_PLACEHOLDER
    } else {
        placeholder
    };
    template.replace(needle, prompt)
}

pub fn contains_prompt_placeholder(template: &str, placeholder: &str) -> bool {
    let needle = if placeholder.is_empty() {
        PROMPT_PLACEHOLDER
    } else {
        placeholder
    };
    template.contains(needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_only_placeholder() {
        let template = r#"{ "model": "gpt-4", "messages": [{ "content": "{{PROMPT}}" }] }"#;
        let body = replace_prompt(template, PROMPT_PLACEHOLDER, "Hello");
        assert!(body.contains("Hello"));
        assert!(body.contains(r#""model": "gpt-4""#));
        assert!(!body.contains(PROMPT_PLACEHOLDER));
    }
}
