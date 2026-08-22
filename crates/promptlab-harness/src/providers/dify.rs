use async_trait::async_trait;

use crate::error::HarnessResult;
use crate::models::{AttackRequest, HttpMethod, NormalizedResponse};
use crate::providers::HttpHarness;
use crate::traits::Harness;

/// Dify `/v1/chat-messages` (blocking JSON or SSE).
#[derive(Clone)]
pub struct DifyHarness {
    inner: HttpHarness,
}

impl DifyHarness {
    pub fn new() -> HarnessResult<Self> {
        Ok(Self {
            inner: HttpHarness::new()?,
        })
    }
}

impl Default for DifyHarness {
    fn default() -> Self {
        Self::new().expect("dify harness")
    }
}

#[async_trait]
impl Harness for DifyHarness {
    fn id(&self) -> &'static str {
        "dify"
    }

    async fn execute(&self, mut request: AttackRequest) -> HarnessResult<NormalizedResponse> {
        request.method = HttpMethod::Post;
        if request.body.is_none() {
            let mode = if request.stream { "streaming" } else { "blocking" };
            request.body = Some(
                serde_json::json!({
                    "query": "{{payload}}",
                    "response_mode": mode,
                    "user": "promptlab-probe",
                    "conversation_id": request.conversation_id.clone().unwrap_or_default(),
                })
                .to_string(),
            );
        } else if request.stream {
            if let Some(body) = request.body.as_mut() {
                *body = body.replace("\"blocking\"", "\"streaming\"");
            }
        }
        let mut response = self.inner.execute_raw(&request, self.id()).await?;
        response
            .metadata
            .insert("api_format".into(), "dify_chat_message".into());
        if let Some(id) = response.conversation_id.clone() {
            response.metadata.insert("conversation_id".into(), id);
        }
        Ok(response)
    }
}
