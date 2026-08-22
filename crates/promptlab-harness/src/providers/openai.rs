use async_trait::async_trait;

use crate::error::HarnessResult;
use crate::models::{AttackRequest, HttpMethod, NormalizedResponse};
use crate::providers::HttpHarness;
use crate::traits::Harness;

/// OpenAI-compatible chat completions harness (also covers Anthropic-style JSON APIs).
#[derive(Clone)]
pub struct OpenAiHarness {
    inner: HttpHarness,
}

impl OpenAiHarness {
    pub fn new() -> HarnessResult<Self> {
        Ok(Self {
            inner: HttpHarness::new()?,
        })
    }
}

impl Default for OpenAiHarness {
    fn default() -> Self {
        Self::new().expect("openai harness")
    }
}

#[async_trait]
impl Harness for OpenAiHarness {
    fn id(&self) -> &'static str {
        "openai"
    }

    async fn execute(&self, mut request: AttackRequest) -> HarnessResult<NormalizedResponse> {
        request.method = HttpMethod::Post;
        if request.body.is_none() {
            if request.has_chat_native() {
                request.body = Some(request.openai_chat_body());
            } else {
                request.body = Some(
                    serde_json::json!({
                        "model": "promptlab-probe",
                        "messages": [
                            { "role": "user", "content": "{{payload}}" }
                        ]
                    })
                    .to_string(),
                );
            }
        }
        if request.url.to_ascii_lowercase().contains("openrouter.ai") {
            if !request
                .headers
                .keys()
                .any(|key| key.eq_ignore_ascii_case("http-referer"))
            {
                request
                    .headers
                    .insert("HTTP-Referer".into(), "https://promptlab.local".into());
            }
            if !request
                .headers
                .keys()
                .any(|key| key.eq_ignore_ascii_case("x-title"))
            {
                request.headers.insert("X-Title".into(), "PromptLab".into());
            }
        }
        let mut response = self.inner.execute_raw(&request, self.id()).await?;
        response.metadata.insert("api_format".into(), "openai_compatible".into());
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AttackRequest;

    #[test]
    fn builds_default_openai_body() {
        let harness = OpenAiHarness::new().unwrap();
        assert_eq!(harness.id(), "openai");
        let request = AttackRequest::from_payload("https://example.com/v1/chat/completions", "probe");
        assert!(request.body.is_none());
    }
}
