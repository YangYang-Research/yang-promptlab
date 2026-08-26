use async_trait::async_trait;

use crate::error::HarnessResult;
use crate::models::{AttackRequest, HttpMethod, NormalizedResponse};
use crate::providers::HttpHarness;
use crate::traits::Harness;

/// OpenAI-compatible chat completions for llama.cpp / Ollama **attack targets**.
///
/// Covers llama-server and Ollama `/v1` endpoints discovered during recon — not
/// product-side embedded inference.
#[derive(Clone)]
pub struct LlamaHarness {
    inner: HttpHarness,
}

impl LlamaHarness {
    pub fn new() -> HarnessResult<Self> {
        Ok(Self {
            inner: HttpHarness::new()?,
        })
    }
}

impl Default for LlamaHarness {
    fn default() -> Self {
        Self::new().expect("llama harness")
    }
}

#[async_trait]
impl Harness for LlamaHarness {
    fn id(&self) -> &'static str {
        "llama"
    }

    async fn execute(&self, mut request: AttackRequest) -> HarnessResult<NormalizedResponse> {
        request.method = HttpMethod::Post;
        if request.url.trim().is_empty() {
            request.url = "http://127.0.0.1:11434/v1/chat/completions".into();
        } else if looks_like_base_url(&request.url) {
            let base = request.url.trim_end_matches('/');
            request.url = format!("{base}/chat/completions");
        }
        if request.body.is_none() {
            request.body = Some(request.openai_chat_body());
        }
        let mut response = self.inner.execute_raw(&request, self.id()).await?;
        response
            .metadata
            .insert("api_format".into(), "openai_compatible".into());
        Ok(response)
    }
}

fn looks_like_base_url(url: &str) -> bool {
    let trimmed = url.trim_end_matches('/');
    !(trimmed.contains("/chat/completions")
        || trimmed.contains("/api/chat")
        || trimmed.contains("/completion"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_is_llama() {
        assert_eq!(LlamaHarness::new().unwrap().id(), "llama");
    }
}
