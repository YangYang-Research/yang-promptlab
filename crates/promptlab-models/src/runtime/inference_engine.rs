use crate::error::{ModelError, ModelResult};
use crate::types::{
    ChatRequest, ChatResponse, EmbeddingRequest, EmbeddingResponse, InferenceRequest,
    InferenceResponse, ModelEntry, ModelProvider, ModelSource,
};

/// Local inference is no longer supported — product AI is remote/third-party only.
/// Callers should use the inference gateway / harness for remote providers.
pub struct LocalInferenceEngine;

const REMOTE_ONLY: &str = "use a remote third-party provider";

impl LocalInferenceEngine {
    pub async fn from_entry(_entry: ModelEntry) -> ModelResult<Self> {
        Err(ModelError::invalid(REMOTE_ONLY))
    }

    pub async fn complete(&self, _request: InferenceRequest) -> ModelResult<InferenceResponse> {
        Err(ModelError::runtime(REMOTE_ONLY))
    }

    pub async fn chat(&self, _request: ChatRequest) -> ModelResult<ChatResponse> {
        Err(ModelError::runtime(REMOTE_ONLY))
    }

    pub async fn embeddings(&self, _request: EmbeddingRequest) -> ModelResult<EmbeddingResponse> {
        Err(ModelError::invalid(REMOTE_ONLY))
    }

    pub async fn health(&self) -> ModelResult<bool> {
        Ok(false)
    }
}

pub fn infer_provider(source: &ModelSource) -> ModelProvider {
    match source {
        ModelSource::Remote { .. } => ModelProvider::Remote,
    }
}

pub fn infer_version(source: &ModelSource) -> String {
    match source {
        ModelSource::Remote { model, .. } => model.clone(),
    }
}

pub fn infer_capabilities(provider: ModelProvider) -> crate::types::ModelCapabilities {
    match provider {
        ModelProvider::Remote => crate::types::ModelCapabilities::remote(),
    }
}

#[cfg(test)]
mod tests {
    fn format_chat_prompt(messages: &[crate::types::ChatMessage]) -> String {
        messages
            .iter()
            .map(|message| {
                let role = message.role.to_lowercase();
                if role == "system" {
                    format!("System: {}\n", message.content)
                } else if role == "assistant" {
                    format!("Assistant: {}\n", message.content)
                } else {
                    format!("User: {}\n", message.content)
                }
            })
            .chain(std::iter::once("Assistant: ".to_string()))
            .collect()
    }

    #[test]
    fn formats_chat_prompt() {
        let prompt = format_chat_prompt(&[crate::types::ChatMessage {
            role: "user".into(),
            content: "Hello".into(),
        }]);
        assert!(prompt.contains("User: Hello"));
        assert!(prompt.ends_with("Assistant: "));
    }
}
