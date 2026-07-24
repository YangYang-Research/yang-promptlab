use std::time::Instant;

use async_trait::async_trait;
use serde_json::json;

use super::bedrock_sigv4;
use super::LlmBackend;
use crate::config::{RemoteProvider, RemoteProviderSettings};
use crate::error::{JudgeError, JudgeResult};

pub struct RemoteLlmBackend {
    settings: RemoteProviderSettings,
    api_key: String,
    aws_secret_access_key: Option<String>,
    client: reqwest::Client,
}

impl RemoteLlmBackend {
    pub fn new(
        settings: RemoteProviderSettings,
        api_key: String,
        aws_secret_access_key: Option<String>,
    ) -> Self {
        Self {
            settings,
            api_key,
            aws_secret_access_key,
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
            RemoteProvider::Nvidia => "https://integrate.api.nvidia.com/v1".into(),
            RemoteProvider::Azure => String::new(),
            RemoteProvider::Bedrock => {
                let region = self
                    .settings
                    .aws_region
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or("us-east-1");
                format!("https://bedrock-runtime.{region}.amazonaws.com")
            }
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
            RemoteProvider::Nvidia => "nvidia",
            RemoteProvider::Azure => "azure",
            RemoteProvider::Bedrock => "bedrock",
        }
    }

    fn model_label(&self) -> &str {
        &self.settings.model
    }

    async fn complete(
        &self,
        system: Option<&str>,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> JudgeResult<String> {
        match self.settings.provider {
            RemoteProvider::Anthropic => {
                self.complete_anthropic(system, prompt, max_tokens, temperature)
                    .await
            }
            RemoteProvider::Gemini => {
                self.complete_gemini(system, prompt, max_tokens, temperature)
                    .await
            }
            RemoteProvider::Bedrock => {
                self.complete_bedrock(system, prompt, max_tokens, temperature)
                    .await
            }
            RemoteProvider::OpenAi
            | RemoteProvider::OpenRouter
            | RemoteProvider::Nvidia
            | RemoteProvider::Azure => {
                self.complete_openai_compatible(system, prompt, max_tokens, temperature)
                    .await
            }
        }
    }

    async fn health_check(&self) -> JudgeResult<bool> {
        let started = Instant::now();
        let sample = self
            .complete(
                Some("Reply with JSON only."),
                r#"{"ok": true}"#,
                32,
                0.0,
            )
            .await?;
        let _ = started;
        Ok(!sample.trim().is_empty())
    }
}

impl RemoteLlmBackend {
    fn compose_prompt(system: Option<&str>, prompt: &str) -> String {
        match system {
            Some(value) if !value.trim().is_empty() => format!("{value}\n\n{prompt}"),
            _ => prompt.to_string(),
        }
    }

    async fn complete_openai_compatible(
        &self,
        system: Option<&str>,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> JudgeResult<String> {
        let url = format!("{}/chat/completions", self.base_url());
        let mut messages = Vec::new();
        if let Some(system) = system.filter(|value| !value.trim().is_empty()) {
            messages.push(json!({"role": "system", "content": system}));
        }
        messages.push(json!({"role": "user", "content": prompt}));
        let body = json!({
            "model": self.settings.model,
            "messages": messages,
            "max_tokens": max_tokens,
            "temperature": temperature,
        });

        let mut req = self.client.post(url).json(&body);
        req = req.header("Authorization", format!("Bearer {}", self.api_key));
        if self.settings.provider == RemoteProvider::OpenRouter {
            req = req.header("HTTP-Referer", "https://promptlab.local");
            req = req.header("X-Title", "PromptLab Judge");
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
        system: Option<&str>,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> JudgeResult<String> {
        let url = format!("{}/messages", self.base_url());
        let mut body = json!({
            "model": self.settings.model,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "messages": [{"role": "user", "content": prompt}],
        });
        if let Some(system) = system.filter(|value| !value.trim().is_empty()) {
            body["system"] = json!(system);
        }

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
        system: Option<&str>,
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
            "contents": [{"parts": [{"text": Self::compose_prompt(system, prompt)}]}],
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

    async fn complete_bedrock(
        &self,
        system: Option<&str>,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> JudgeResult<String> {
        let secret = self.aws_secret_access_key.as_deref().ok_or_else(|| {
            JudgeError::config("bedrock requires a secret access key")
        })?;
        let access_key_id = self.api_key.trim();
        let secret_access_key = secret.trim();
        let region = self
            .settings
            .aws_region
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("us-east-1")
            .trim();
        let model_id = self.settings.model.trim();
        let path = bedrock_sigv4::bedrock_converse_path(model_id);
        let host = format!("bedrock-runtime.{region}.amazonaws.com");
        // Use the raw model id in the request URL so reqwest encodes `:` once on the wire.
        // Pre-encoded `%3A` in the URL string is double-encoded to `%253A` and breaks SigV4.
        let url = format!("https://{host}/model/{model_id}/converse");
        let body = json!({
            "messages": [{
                "role": "user",
                "content": [{"text": Self::compose_prompt(system, prompt)}]
            }],
            "inferenceConfig": {
                "maxTokens": max_tokens,
                "temperature": temperature,
            }
        });
        let body_str = serde_json::to_string(&body)
            .map_err(|e| JudgeError::evaluation(format!("bedrock body encode failed: {e}")))?;
        let session_token = self.settings.aws_session_token.trim();
        let session_token = if session_token.is_empty() {
            None
        } else {
            Some(session_token)
        };
        let signed = bedrock_sigv4::sign_bedrock_post(
            &host,
            &path,
            &body_str,
            access_key_id,
            secret_access_key,
            region,
            session_token,
        )
        .map_err(|e| JudgeError::evaluation(format!("bedrock signing failed: {e}")))?;

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
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| JudgeError::evaluation(format!("bedrock request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(JudgeError::evaluation(format!(
                "bedrock returned {status}: {}",
                summarize_bedrock_error(&text)
            )));
        }

        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| JudgeError::evaluation(format!("bedrock parse failed: {e}")))?;

        value
            .pointer("/output/message/content/0/text")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| JudgeError::evaluation("bedrock returned empty content"))
    }
}

fn summarize_bedrock_error(body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(message) = value.get("message").and_then(|v| v.as_str()) {
            if message.contains("The request signature we calculated does not match") {
                return "Signature mismatch — verify access key, secret, session token, and region"
                    .into();
            }
            if message.contains("The security token included in the request is invalid") {
                return "Invalid or expired AWS credentials — refresh access key, secret, and session token"
                    .into();
            }
            if message.len() <= 400 {
                return message.to_string();
            }
            return format!("{}…", &message[..400]);
        }
    }

    if body.len() <= 400 {
        body.to_string()
    } else {
        format!("{}…", &body[..400])
    }
}
