use async_trait::async_trait;

use crate::error::HarnessResult;
use crate::models::{AttackRequest, HttpMethod, NormalizedResponse};
use crate::providers::HttpHarness;
use crate::traits::Harness;

/// Anthropic Messages API (`POST /v1/messages`).
#[derive(Clone)]
pub struct AnthropicHarness {
    inner: HttpHarness,
}

impl AnthropicHarness {
    pub fn new() -> HarnessResult<Self> {
        Ok(Self {
            inner: HttpHarness::new()?,
        })
    }
}

impl Default for AnthropicHarness {
    fn default() -> Self {
        Self::new().expect("anthropic harness")
    }
}

#[async_trait]
impl Harness for AnthropicHarness {
    fn id(&self) -> &'static str {
        "anthropic"
    }

    async fn execute(&self, mut request: AttackRequest) -> HarnessResult<NormalizedResponse> {
        request.method = HttpMethod::Post;
        if !request
            .headers
            .keys()
            .any(|k| k.eq_ignore_ascii_case("anthropic-version"))
        {
            request
                .headers
                .insert("anthropic-version".into(), "2023-06-01".into());
        }
        if request.auth.api_key_header.is_none() && request.auth.api_key.is_some() {
            request.auth.api_key_header = Some("x-api-key".into());
        }
        if request.auth.api_key.is_none() {
            if let Some(token) = request.auth.bearer_token.clone() {
                request.auth.api_key = Some(token);
                request.auth.api_key_header = Some("x-api-key".into());
                request.auth.bearer_token = None;
            }
        }
        if request.body.is_none() {
            if request.has_chat_native() {
                request.body = Some(request.anthropic_chat_body());
            } else {
                request.body = Some(
                    serde_json::json!({
                        "model": "claude-3-5-sonnet-20241022",
                        "max_tokens": 256,
                        "messages": [
                            { "role": "user", "content": "{{payload}}" }
                        ]
                    })
                    .to_string(),
                );
            }
        }
        let mut response = self.inner.execute_raw(&request, self.id()).await?;
        response
            .metadata
            .insert("api_format".into(), "anthropic_messages".into());
        Ok(response)
    }
}
