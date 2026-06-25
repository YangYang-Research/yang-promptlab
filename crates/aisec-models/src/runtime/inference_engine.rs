use std::time::Instant;

use crate::error::{ModelError, ModelResult};
use crate::runtime::{InferenceRuntime, LlamaInProcessRuntime, LlamaModelConfig, OllamaRuntime};
use crate::types::{
    ChatRequest, ChatResponse, EmbeddingRequest, EmbeddingResponse, InferenceRequest,
    InferenceResponse, ModelEntry, ModelProvider, ModelSource,
};

/// Unified local inference engine routing to Ollama or embedded libllama based on model entry.
pub struct LocalInferenceEngine {
    entry: ModelEntry,
    ollama: Option<OllamaRuntime>,
    llama: Option<LlamaInProcessRuntime>,
}

impl LocalInferenceEngine {
    pub async fn from_entry(entry: ModelEntry, llama_config: LlamaModelConfig) -> ModelResult<Self> {
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
                    llama: None,
                })
            }
            ModelProvider::Remote => Err(ModelError::invalid(
                "remote cloud models use the third-party provider API, not local inference",
            )),
            ModelProvider::HuggingFace | ModelProvider::Gguf => {
                if !entry.file_path.exists() {
                    return Err(ModelError::invalid(format!(
                        "model file missing: {}",
                        entry.file_path.display()
                    )));
                }
                let mut runtime = LlamaInProcessRuntime::new(llama_config);
                runtime.load_model(&entry.file_path).await?;
                Ok(Self {
                    entry,
                    ollama: None,
                    llama: Some(runtime),
                })
            }
        }
    }

    pub fn entry(&self) -> &ModelEntry {
        &self.entry
    }

    pub async fn complete(&self, request: InferenceRequest) -> ModelResult<InferenceResponse> {
        if let Some(runtime) = &self.ollama {
            return runtime.complete(request).await;
        }
        if let Some(runtime) = &self.llama {
            return runtime.complete(request).await;
        }
        Err(ModelError::runtime("no runtime loaded"))
    }

    pub async fn chat(&self, request: ChatRequest) -> ModelResult<ChatResponse> {
        let started = Instant::now();
        if let Some(runtime) = &self.ollama {
            return runtime.chat(request).await;
        }
        if let Some(runtime) = &self.llama {
            let prompt = format_chat_prompt(&request.messages);
            let response = runtime
                .complete(InferenceRequest {
                    prompt,
                    max_tokens: request.max_tokens,
                    temperature: request.temperature,
                })
                .await?;
            return Ok(ChatResponse {
                message: crate::types::ChatMessage {
                    role: "assistant".into(),
                    content: response.text,
                },
                tokens_predicted: response.tokens_predicted,
                duration_ms: started.elapsed().as_millis() as u64,
            });
        }
        Err(ModelError::runtime("no runtime loaded"))
    }

    pub async fn embeddings(&self, request: EmbeddingRequest) -> ModelResult<EmbeddingResponse> {
        if let Some(runtime) = &self.ollama {
            return runtime.embeddings(request).await;
        }
        Err(ModelError::invalid(
            "embeddings require a dedicated embedding model — capability unavailable for this GGUF",
        ))
    }

    pub async fn health(&self) -> ModelResult<bool> {
        if let Some(runtime) = &self.ollama {
            return runtime.health().await;
        }
        if let Some(runtime) = &self.llama {
            return runtime.health().await;
        }
        Ok(false)
    }
}

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

pub fn infer_provider(source: &ModelSource) -> ModelProvider {
    match source {
        ModelSource::Ollama { .. } => ModelProvider::Ollama,
        ModelSource::HuggingFace { .. } => ModelProvider::HuggingFace,
        ModelSource::Local { .. } => ModelProvider::Gguf,
        ModelSource::Remote { .. } => ModelProvider::Remote,
    }
}

pub fn infer_version(source: &ModelSource) -> String {
    match source {
        ModelSource::HuggingFace { revision, filename, .. } => revision
            .clone()
            .unwrap_or_else(|| filename.clone()),
        ModelSource::Ollama { model, .. } => model.clone(),
        ModelSource::Remote { model, .. } => model.clone(),
        ModelSource::Local { path } => path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("local")
            .into(),
    }
}

pub fn infer_capabilities(provider: ModelProvider) -> crate::types::ModelCapabilities {
    match provider {
        ModelProvider::Ollama => crate::types::ModelCapabilities::ollama(),
        ModelProvider::HuggingFace | ModelProvider::Gguf | ModelProvider::Remote => {
            crate::types::ModelCapabilities::gguf()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
