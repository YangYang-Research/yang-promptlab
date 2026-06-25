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
        let sample = self
            .complete(
                Some(PromptRegistry::health_check_system()),
                PromptRegistry::health_check_user(),
                32,
                0.0,
            )
            .await?;
        Ok(!sample.trim().is_empty())
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
        let url = format!("{}/chat/completions", self.base_url());
        let body = json!({
            "model": self.settings.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": prompt}
            ],
            "max_tokens": max_tokens,
            "temperature": temperature,
        });

        let mut req = self.client.post(url).json(&body);
        req = req.header("Authorization", format!("Bearer {}", self.settings.api_key));
        if self.settings.provider == InferenceProvider::OpenRouter {
            req = req.header("HTTP-Referer", "https://aisec.local");
            req = req.header("X-Title", "AISec");
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

        value
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| InferenceError::Provider("remote llm returned empty content".into()))
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
