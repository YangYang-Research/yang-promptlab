use std::time::Instant;

use async_trait::async_trait;
use serde_json::json;

use super::bedrock_sigv4;
use super::{ProviderAdapter, RemoteAdapterSettings};
use crate::capabilities::ModelCapabilities;
use crate::config::InferenceProvider;
use crate::error::{InferenceError, InferenceResult};
use crate::prompts::PromptRegistry;
use crate::types::ChatMessage;

pub struct RemoteProviderAdapter {
    settings: RemoteAdapterSettings,
    client: reqwest::Client,
}

impl RemoteProviderAdapter {
    pub fn new(settings: RemoteAdapterSettings) -> Self {
        Self {
            settings,
            client: reqwest::Client::new(),
        }
    }

    fn base_url(&self) -> String {
        if let Some(url) = &self.settings.base_url {
            if !url.trim().is_empty() {
                return url.trim().trim_end_matches('/').to_string();
            }
        }
        match self.settings.provider {
            InferenceProvider::OpenAi => "https://api.openai.com/v1".into(),
            InferenceProvider::Anthropic => "https://api.anthropic.com/v1".into(),
            InferenceProvider::Gemini => "https://generativelanguage.googleapis.com/v1beta".into(),
            InferenceProvider::OpenRouter => "https://openrouter.ai/api/v1".into(),
            InferenceProvider::Azure => String::new(),
            InferenceProvider::Bedrock => {
                let region = self
                    .settings
                    .aws_region
                    .as_deref()
                    .filter(|v| !v.trim().is_empty())
                    .unwrap_or("us-east-1");
                format!("https://bedrock-runtime.{region}.amazonaws.com")
            }
            _ => "https://api.openai.com/v1".into(),
        }
    }

    fn is_openrouter_endpoint(&self) -> bool {
        self.base_url().to_ascii_lowercase().contains("openrouter.ai")
    }

    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(120)
    }
}

#[async_trait]
impl ProviderAdapter for RemoteProviderAdapter {
    fn provider_id(&self) -> &str {
        self.settings.provider.as_str()
    }

    fn model_id(&self) -> &str {
        &self.settings.model
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities::from_remote(self.provider_id())
    }

    async fn complete(
        &self,
        system: Option<&str>,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> InferenceResult<String> {
        let system = system.unwrap_or(PromptRegistry::inference_system());
        match self.settings.provider {
            InferenceProvider::Anthropic => {
                self.complete_anthropic(system, prompt, max_tokens, temperature)
                    .await
            }
            InferenceProvider::Gemini => {
                self.complete_gemini(prompt, max_tokens, temperature).await
            }
            InferenceProvider::Bedrock => {
                self.complete_bedrock(prompt, max_tokens, temperature).await
            }
            _ => {
                self.complete_openai_compatible(system, prompt, max_tokens, temperature)
                    .await
            }
        }
    }

    async fn chat(
        &self,
        messages: &[ChatMessage],
        max_tokens: u32,
        temperature: f32,
    ) -> InferenceResult<String> {
        let system = messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.as_str());
        let user = messages
            .iter()
            .filter(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        self.complete(system, &user, max_tokens, temperature).await
    }

    async fn health(&self) -> InferenceResult<bool> {
        match self.settings.provider {
            InferenceProvider::Anthropic => {
                let sample = self
                    .complete_anthropic(
                        PromptRegistry::health_check_system(),
                        PromptRegistry::health_check_user(),
                        32,
                        0.0,
                    )
                    .await?;
                Ok(!sample.trim().is_empty())
            }
            InferenceProvider::Gemini => {
                let sample = self
                    .complete_gemini(PromptRegistry::health_check_user(), 32, 0.0)
                    .await?;
                Ok(!sample.trim().is_empty())
            }
            InferenceProvider::Bedrock => {
                let sample = self
                    .complete_bedrock(PromptRegistry::health_check_user(), 32, 0.0)
                    .await?;
                Ok(!sample.trim().is_empty())
            }
            InferenceProvider::OpenRouter => self.probe_openai_compatible_connectivity().await,
            _ if self.is_openrouter_endpoint() => self.probe_openai_compatible_connectivity().await,
            _ => {
                let sample = self
                    .complete_openai_compatible_user_only("Reply with exactly: OK", 64, 0.0)
                    .await?;
                Ok(!sample.trim().is_empty())
            }
        }
    }
}

impl RemoteProviderAdapter {
    async fn complete_openai_compatible(
        &self,
        system: &str,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> InferenceResult<String> {
        let mut messages = Vec::new();
        if !system.trim().is_empty() {
            messages.push(json!({"role": "system", "content": system}));
        }
        messages.push(json!({"role": "user", "content": prompt}));
        self.post_openai_compatible_chat(messages, max_tokens, temperature)
            .await
    }

    async fn complete_openai_compatible_user_only(
        &self,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> InferenceResult<String> {
        let messages = vec![json!({"role": "user", "content": prompt})];
        self.post_openai_compatible_chat(messages, max_tokens, temperature)
            .await
    }

    async fn post_openai_compatible_chat(
        &self,
        messages: Vec<serde_json::Value>,
        max_tokens: u32,
        temperature: f32,
    ) -> InferenceResult<String> {
        let url = format!("{}/chat/completions", self.base_url());
        let mut body = json!({
            "model": self.settings.model,
            "messages": messages,
            "max_tokens": max_tokens,
            "temperature": temperature,
        });
        if self.settings.provider == InferenceProvider::OpenRouter || self.is_openrouter_endpoint() {
            body["include_reasoning"] = json!(true);
        }

        let mut req = self.client.post(url).json(&body);
        req = req.header("Authorization", format!("Bearer {}", self.settings.api_key));
        if self.settings.provider == InferenceProvider::OpenRouter || self.is_openrouter_endpoint() {
            req = req.header("HTTP-Referer", "https://aisec.local");
            req = req.header("X-Title", "PromptLab");
        }

        let response = req
            .timeout(self.timeout())
            .send()
            .await
            .map_err(|e| InferenceError::Provider(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(InferenceError::Provider(format!(
                "remote llm returned {status}: {text}"
            )));
        }

        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| InferenceError::Provider(e.to_string()))?;

        extract_openai_chat_content(&value).ok_or_else(|| {
            InferenceError::Provider("remote llm returned empty content".into())
        })
    }

    async fn probe_openai_compatible_connectivity(&self) -> InferenceResult<bool> {
        let url = format!("{}/chat/completions", self.base_url());
        let body = json!({
            "model": self.settings.model,
            "messages": [{"role": "user", "content": "OK"}],
            "max_tokens": 16,
            "temperature": 0.0,
            "reasoning": {"effort": "minimal"},
        });

        let mut req = self.client.post(url).json(&body);
        req = req.header("Authorization", format!("Bearer {}", self.settings.api_key));
        req = req.header("HTTP-Referer", "https://aisec.local");
        req = req.header("X-Title", "PromptLab");

        let response = req
            .timeout(self.timeout())
            .send()
            .await
            .map_err(|e| InferenceError::Provider(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(InferenceError::Provider(format!(
                "remote llm returned {status}: {text}"
            )));
        }

        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| InferenceError::Provider(e.to_string()))?;

        if extract_openai_chat_content(&value).is_some() {
            return Ok(true);
        }

        Ok(connectivity_response_has_completion(&value))
    }

    async fn complete_anthropic(
        &self,
        system: &str,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> InferenceResult<String> {
        let url = format!("{}/messages", self.base_url());
        let body = json!({
            "model": self.settings.model,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "system": system,
            "messages": [{"role": "user", "content": prompt}],
        });

        let response = self
            .client
            .post(url)
            .header("x-api-key", &self.settings.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .timeout(self.timeout())
            .send()
            .await
            .map_err(|e| InferenceError::Provider(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(InferenceError::Provider(format!(
                "anthropic returned {status}: {text}"
            )));
        }

        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| InferenceError::Provider(e.to_string()))?;

        value
            .pointer("/content/0/text")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| InferenceError::Provider("anthropic returned empty content".into()))
    }

    async fn complete_gemini(
        &self,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> InferenceResult<String> {
        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.base_url(),
            self.settings.model,
            self.settings.api_key
        );
        let body = json!({
            "contents": [{"parts": [{"text": prompt}]}],
            "generationConfig": {
                "temperature": temperature,
                "maxOutputTokens": max_tokens,
            }
        });

        let response = self
            .client
            .post(url)
            .json(&body)
            .timeout(self.timeout())
            .send()
            .await
            .map_err(|e| InferenceError::Provider(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(InferenceError::Provider(format!(
                "gemini returned {status}: {text}"
            )));
        }

        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| InferenceError::Provider(e.to_string()))?;

        value
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| InferenceError::Provider("gemini returned empty content".into()))
    }

    async fn complete_bedrock(
        &self,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> InferenceResult<String> {
        let secret = self.settings.aws_secret_access_key.as_deref().ok_or_else(|| {
            InferenceError::Config("bedrock requires a secret access key".into())
        })?;
        let region = self
            .settings
            .aws_region
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or("us-east-1")
            .trim();
        let model_id = self.settings.model.trim();
        let path = bedrock_sigv4::bedrock_converse_path(model_id);
        let host = format!("bedrock-runtime.{region}.amazonaws.com");
        let url = format!("https://{host}/model/{model_id}/converse");
        let body = json!({
            "messages": [{
                "role": "user",
                "content": [{"text": prompt}]
            }],
            "inferenceConfig": {
                "maxTokens": max_tokens,
                "temperature": temperature,
            }
        });
        let body_str = serde_json::to_string(&body)
            .map_err(|e| InferenceError::Serialization(e.to_string()))?;
        let session_token = self
            .settings
            .aws_session_token
            .as_deref()
            .filter(|v| !v.trim().is_empty());
        let signed = bedrock_sigv4::sign_bedrock_post(
            &host,
            &path,
            &body_str,
            self.settings.api_key.trim(),
            secret.trim(),
            region,
            session_token,
        )
        .map_err(InferenceError::Provider)?;

        let mut request = self
            .client
            .post(&url)
            .header("content-type", "application/json")
            .header("authorization", signed.authorization)
            .header("x-amz-date", signed.amz_date)
            .header("x-amz-content-sha256", signed.payload_hash);
        if let Some(token) = signed.session_token {
            request = request.header("x-amz-security-token", token);
        }

        let response = request
            .body(body_str)
            .timeout(self.timeout())
            .send()
            .await
            .map_err(|e| InferenceError::Provider(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(InferenceError::Provider(format!(
                "bedrock returned {status}: {text}"
            )));
        }

        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| InferenceError::Provider(e.to_string()))?;

        value
            .pointer("/output/message/content/0/text")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| InferenceError::Provider("bedrock returned empty content".into()))
    }
}

fn push_non_empty_text(out: &mut String, text: &str) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(trimmed);
}

fn text_from_json_value(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .or_else(|| {
            if let Some(parts) = value.as_array() {
                let mut out = String::new();
                for part in parts {
                    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                        push_non_empty_text(&mut out, text);
                    }
                }
                if out.trim().is_empty() {
                    None
                } else {
                    Some(out)
                }
            } else {
                None
            }
        })
}

fn extract_openai_chat_content(value: &serde_json::Value) -> Option<String> {
    let message = value.pointer("/choices/0/message")?;

    for key in ["content", "reasoning", "reasoning_content"] {
        if let Some(field) = message.get(key) {
            if !field.is_null() {
                if let Some(text) = text_from_json_value(field) {
                    return Some(text);
                }
            }
        }
    }

    if let Some(details) = message.get("reasoning_details").and_then(|v| v.as_array()) {
        let mut out = String::new();
        for item in details {
            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                push_non_empty_text(&mut out, text);
            }
        }
        if !out.trim().is_empty() {
            return Some(out);
        }
    }

    if let Some(text) = value.pointer("/choices/0/text").and_then(|v| v.as_str()) {
        if !text.trim().is_empty() {
            return Some(text.to_string());
        }
    }

    None
}

fn connectivity_response_has_completion(value: &serde_json::Value) -> bool {
    let finish_ok = value
        .pointer("/choices/0/finish_reason")
        .and_then(|v| v.as_str())
        .is_some_and(|reason| reason == "stop" || reason == "length");
    let tokens = value
        .pointer("/usage/completion_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    finish_ok && tokens > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_openai_chat_content_reads_string_content() {
        let value = json!({"choices":[{"message":{"content":"hello"}}]});
        assert_eq!(extract_openai_chat_content(&value).as_deref(), Some("hello"));
    }

    #[test]
    fn extract_openai_chat_content_reads_reasoning_when_content_empty() {
        let value = json!({"choices":[{"message":{"content":"","reasoning":"thinking"}}]});
        assert_eq!(
            extract_openai_chat_content(&value).as_deref(),
            Some("thinking")
        );
    }

    #[test]
    fn extract_openai_chat_content_reads_array_content() {
        let value = json!({"choices":[{"message":{"content":[{"type":"text","text":"hello"}]}}]});
        assert_eq!(extract_openai_chat_content(&value).as_deref(), Some("hello"));
    }

    #[test]
    fn extract_openai_chat_content_reads_reasoning_details() {
        let value = json!({"choices":[{"message":{"content":null,"reasoning_details":[{"type":"reasoning.text","text":"thinking"}]}}]});
        assert_eq!(
            extract_openai_chat_content(&value).as_deref(),
            Some("thinking")
        );
    }

    #[test]
    fn connectivity_response_has_completion_accepts_openrouter_usage() {
        let value = json!({
            "choices":[{"finish_reason":"stop","message":{"content":"hello"}}],
            "usage":{"completion_tokens":12}
        });
        assert!(connectivity_response_has_completion(&value));
    }
}
