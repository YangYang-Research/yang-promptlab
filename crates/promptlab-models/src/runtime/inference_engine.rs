use std::time::Instant;

use crate::error::{ModelError, ModelResult};
use crate::runtime::{InferenceRuntime, OllamaRuntime};
use crate::types::{
    ChatRequest, ChatResponse, EmbeddingRequest, EmbeddingResponse, InferenceRequest,
    InferenceResponse, ModelEntry, ModelProvider, ModelSource,
};

/// Unified inference engine — Ollama HTTP only.
pub struct LocalInferenceEngine {
    entry: ModelEntry,
    ollama: Option<OllamaRuntime>,
}

impl LocalInferenceEngine {
    pub async fn from_entry(entry: ModelEntry) -> ModelResult<Self> {
        match entry.provider {
            ModelProvider::Ollama => {
                let (model, base_url) = match &entry.source {
                    ModelSource::Ollama { model, base_url } => (model.clone(), base_url.clone()),
                    _ => {
                        return Err(ModelError::invalid(
                            "ollama entry missing ollama source metadata",
                        ));
                    }
                };
                Ok(Self {
                    entry,
                    ollama: Some(OllamaRuntime::new(crate::runtime::OllamaConfig {
                        base_url,
                        model,
                    })),
                })
            }
            ModelProvider::Remote => Err(ModelError::invalid(
                "remote cloud models use the third-party provider API, not local inference",
            )),
        }
    }

    pub fn entry(&self) -> &ModelEntry {
        &self.entry
    }

    pub async fn complete(&self, request: InferenceRequest) -> ModelResult<InferenceResponse> {
        if let Some(runtime) = &self.ollama {
            return runtime.complete(request).await;
        }
        Err(ModelError::runtime("no runtime loaded"))
    }

    pub async fn chat(&self, request: ChatRequest) -> ModelResult<ChatResponse> {
        let _started = Instant::now();
        if let Some(runtime) = &self.ollama {
            return runtime.chat(request).await;
        }
        Err(ModelError::runtime("no runtime loaded"))
    }

    pub async fn embeddings(&self, request: EmbeddingRequest) -> ModelResult<EmbeddingResponse> {
        if let Some(runtime) = &self.ollama {
            return runtime.embeddings(request).await;
        }
        Err(ModelError::invalid(
            "embeddings require an Ollama embedding model over HTTP",
        ))
    }

    pub async fn health(&self) -> ModelResult<bool> {
        if let Some(runtime) = &self.ollama {
            return runtime.health().await;
        }
        Ok(false)
    }
}

pub fn infer_provider(source: &ModelSource) -> ModelProvider {
    match source {
        ModelSource::Ollama { .. } => ModelProvider::Ollama,
        ModelSource::Remote { .. } => ModelProvider::Remote,
    }
}

pub fn infer_version(source: &ModelSource) -> String {
    match source {
        ModelSource::Ollama { model, .. } => model.clone(),
        ModelSource::Remote { model, .. } => model.clone(),
    }
}

pub fn infer_capabilities(provider: ModelProvider) -> crate::types::ModelCapabilities {
    match provider {
        ModelProvider::Ollama => crate::types::ModelCapabilities::ollama(),
        ModelProvider::Remote => crate::types::ModelCapabilities::remote(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
