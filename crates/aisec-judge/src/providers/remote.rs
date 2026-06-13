use std::time::Instant;

use async_trait::async_trait;
use serde_json::json;

use super::LlmBackend;
use crate::config::{RemoteProvider, RemoteProviderSettings};
use crate::error::{JudgeError, JudgeResult};

pub struct RemoteLlmBackend {
    settings: RemoteProviderSettings,
    api_key: String,
    client: reqwest::Client,
}

impl RemoteLlmBackend {
    pub fn new(settings: RemoteProviderSettings, api_key: String) -> Self {
        Self {
            settings,
            api_key,
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
            RemoteProvider::OpenAi => "https://api.openai.com/v1".into(),
            RemoteProvider::Anthropic => "https://api.anthropic.com/v1".into(),
            RemoteProvider::Gemini => {
                "https://generativelanguage.googleapis.com/v1beta".into()
            }
            RemoteProvider::OpenRouter => "https://openrouter.ai/api/v1".into(),
        }
    }
}

#[async_trait]
impl LlmBackend for RemoteLlmBackend {
    fn provider_label(&self) -> &str {
        match self.settings.provider {
            RemoteProvider::OpenAi => "openai",
            RemoteProvider::Anthropic => "anthropic",
            RemoteProvider::Gemini => "gemini",
            RemoteProvider::OpenRouter => "openrouter",
        }
    }

    fn model_label(&self) -> &str {
        &self.settings.model
    }

    async fn complete(&self, prompt: &str, max_tokens: u32, temperature: f32) -> JudgeResult<String> {
        match self.settings.provider {
            RemoteProvider::Anthropic => {
                self.complete_anthropic(prompt, max_tokens, temperature)
                    .await
            }
            RemoteProvider::Gemini => self.complete_gemini(prompt, max_tokens, temperature).await,
            RemoteProvider::OpenAi | RemoteProvider::OpenRouter => {
                self.complete_openai_compatible(prompt, max_tokens, temperature)
                    .await
            }
        }
    }

    async fn health_check(&self) -> JudgeResult<bool> {
        let started = Instant::now();
        let sample = self
            .complete(
                "Reply with JSON only: {\"ok\": true}",
                32,
                0.0,
            )
            .await?;
        let _ = started;
        Ok(!sample.trim().is_empty())
    }
}

impl RemoteLlmBackend {
    async fn complete_openai_compatible(
        &self,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> JudgeResult<String> {
        let url = format!("{}/chat/completions", self.base_url());
        let body = json!({
            "model": self.settings.model,
            "messages": [
                {"role": "system", "content": "You are a security judge. Respond with JSON only."},
                {"role": "user", "content": prompt}
            ],
            "max_tokens": max_tokens,
            "temperature": temperature,
        });

        let mut req = self.client.post(url).json(&body);
        req = req.header("Authorization", format!("Bearer {}", self.api_key));
        if self.settings.provider == RemoteProvider::OpenRouter {
            req = req.header("HTTP-Referer", "https://aisec.local");
            req = req.header("X-Title", "AISec Judge");
        }

        let response = req
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| JudgeError::evaluation(format!("remote llm request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(JudgeError::evaluation(format!(
                "remote llm returned {status}: {text}"
            )));
        }

        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| JudgeError::evaluation(format!("remote llm parse failed: {e}")))?;

        value
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| JudgeError::evaluation("remote llm returned empty content"))
    }

    async fn complete_anthropic(
        &self,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> JudgeResult<String> {
        let url = format!("{}/messages", self.base_url());
        let body = json!({
            "model": self.settings.model,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "system": "You are a security judge. Respond with JSON only.",
            "messages": [{"role": "user", "content": prompt}],
        });

        let response = self
            .client
            .post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| JudgeError::evaluation(format!("anthropic request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(JudgeError::evaluation(format!(
                "anthropic returned {status}: {text}"
            )));
        }

        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| JudgeError::evaluation(format!("anthropic parse failed: {e}")))?;

        value
            .pointer("/content/0/text")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| JudgeError::evaluation("anthropic returned empty content"))
    }

    async fn complete_gemini(
        &self,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> JudgeResult<String> {
        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.base_url(),
            self.settings.model,
            self.api_key
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
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| JudgeError::evaluation(format!("gemini request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(JudgeError::evaluation(format!(
                "gemini returned {status}: {text}"
            )));
        }

        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| JudgeError::evaluation(format!("gemini parse failed: {e}")))?;

        value
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| JudgeError::evaluation("gemini returned empty content"))
    }
}
