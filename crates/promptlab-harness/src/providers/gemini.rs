use async_trait::async_trait;

use crate::error::HarnessResult;
use crate::models::{AttackRequest, HttpMethod, NormalizedResponse};
use crate::providers::HttpHarness;
use crate::traits::Harness;

/// Google Gemini `generateContent` / `streamGenerateContent`.
#[derive(Clone)]
pub struct GeminiHarness {
    inner: HttpHarness,
}

impl GeminiHarness {
    pub fn new() -> HarnessResult<Self> {
        Ok(Self {
            inner: HttpHarness::new()?,
        })
    }
}

impl Default for GeminiHarness {
    fn default() -> Self {
        Self::new().expect("gemini harness")
    }
}

#[async_trait]
impl Harness for GeminiHarness {
    fn id(&self) -> &'static str {
        "gemini"
    }

    async fn execute(&self, mut request: AttackRequest) -> HarnessResult<NormalizedResponse> {
        request.method = HttpMethod::Post;
        if request.auth.query_key_value.is_none() {
            if let Some(key) = request
                .auth
                .api_key
                .clone()
                .or_else(|| request.auth.bearer_token.clone())
            {
                request.auth.query_key_name = Some("key".into());
                request.auth.query_key_value = Some(key);
            }
        }
        if request.stream && !request.url.contains("streamGenerateContent") {
            request.url = request.url.replace("generateContent", "streamGenerateContent");
        }
        if request.body.is_none() {
            if request.has_chat_native() {
                request.body = Some(request.gemini_chat_body());
            } else {
                request.body = Some(
                    serde_json::json!({
                        "contents": [{
                            "parts": [{ "text": "{{payload}}" }]
                        }]
                    })
                    .to_string(),
                );
            }
        }
        let mut response = self.inner.execute_raw(&request, self.id()).await?;
        response
            .metadata
            .insert("api_format".into(), "gemini_generate_content".into());
        Ok(response)
    }
}
