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
            request.body = Some(
                serde_json::json!({
                    "model": "aisec-probe",
                    "messages": [
                        { "role": "user", "content": "{{payload}}" }
                    ]
                })
                .to_string(),
            );
        }
        let mut response = self.inner.execute(request).await?;
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
