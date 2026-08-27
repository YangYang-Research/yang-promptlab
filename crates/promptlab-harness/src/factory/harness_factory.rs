use std::sync::{Arc, RwLock};

use crate::error::{HarnessError, HarnessResult};
use crate::models::{HarnessKind, TargetDescriptor};
use crate::pipeline::{HarnessInterceptor, InterceptAction};
use crate::providers::{
    AnthropicHarness, BedrockHarness, DifyHarness, GeminiHarness, HttpHarness, LlamaHarness,
    McpHarness, OpenAiHarness, WebSocketHarness,
};
#[cfg(feature = "playwright")]
use crate::providers::PlaywrightHarness;
use crate::registry::HarnessRegistry;
use crate::traits::{DefaultResponseNormalizer, Harness, ResponseNormalizer};

/// App-wide target I/O bus. Callers pass a [`TargetDescriptor`] + request;
/// new protocols [`register`](Self::register) here instead of growing per-feature HTTP clients.
#[derive(Clone)]
pub struct HarnessFactory {
    registry: Arc<RwLock<HarnessRegistry>>,
    interceptors: Arc<RwLock<Vec<Arc<dyn HarnessInterceptor>>>>,
    normalizer: DefaultResponseNormalizer,
}

impl HarnessFactory {
    pub fn new() -> HarnessResult<Self> {
        let mut registry = HarnessRegistry::new();
        registry.register(Arc::new(HttpHarness::new()?));
        registry.register(Arc::new(OpenAiHarness::new()?));
        registry.register(Arc::new(AnthropicHarness::new()?));
        registry.register(Arc::new(GeminiHarness::new()?));
        registry.register(Arc::new(DifyHarness::new()?));
        registry.register(Arc::new(McpHarness::new()?));
        registry.register(Arc::new(WebSocketHarness::new()));
        registry.register(Arc::new(BedrockHarness::new()?));
        registry.register(Arc::new(LlamaHarness::new()?));
        Ok(Self {
            registry: Arc::new(RwLock::new(registry)),
            interceptors: Arc::new(RwLock::new(Vec::new())),
            normalizer: DefaultResponseNormalizer,
        })
    }

    /// Build a factory from a pre-populated registry (tests and custom runtimes).
    pub fn from_registry(registry: HarnessRegistry) -> Self {
        Self {
            registry: Arc::new(RwLock::new(registry)),
            interceptors: Arc::new(RwLock::new(Vec::new())),
            normalizer: DefaultResponseNormalizer,
        }
    }

    /// Clone registry and interceptor list so per-scan Playwright/plugin state
    /// cannot leak onto the shared AppState factory.
    pub fn isolated(&self) -> Self {
        let interceptors = self
            .interceptors
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        let registry = self
            .registry
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        Self {
            registry: Arc::new(RwLock::new(registry)),
            interceptors: Arc::new(RwLock::new(interceptors)),
            normalizer: self.normalizer,
        }
    }

    pub fn registry(&self) -> Arc<RwLock<HarnessRegistry>> {
        self.registry.clone()
    }

    pub fn register(&self, harness: Arc<dyn Harness>) -> HarnessResult<()> {
        self.registry
            .write()
            .map_err(|_| HarnessError::config("harness registry lock poisoned"))?
            .register(harness);
        Ok(())
    }

    pub fn add_interceptor(&self, interceptor: Arc<dyn HarnessInterceptor>) -> HarnessResult<()> {
        self.interceptors
            .write()
            .map_err(|_| HarnessError::config("harness interceptor lock poisoned"))?
            .push(interceptor);
        Ok(())
    }

    pub fn registered_ids(&self) -> HarnessResult<Vec<String>> {
        Ok(self
            .registry
            .read()
            .map_err(|_| HarnessError::config("harness registry lock poisoned"))?
            .ids())
    }

    #[cfg(feature = "playwright")]
    pub fn with_playwright(self, harness: PlaywrightHarness) -> Self {
        if let Ok(mut registry) = self.registry.write() {
            registry.register(Arc::new(harness));
        }
        self
    }

    pub fn resolve(&self, descriptor: &TargetDescriptor) -> HarnessResult<Arc<dyn Harness>> {
        self.resolve_kind(descriptor.preferred_harness())
    }

    pub fn resolve_by_id(&self, harness_id: &str) -> HarnessResult<Arc<dyn Harness>> {
        self.registry
            .read()
            .map_err(|_| HarnessError::config("harness registry lock poisoned"))?
            .get(harness_id)
    }

    pub fn resolve_kind(&self, kind: HarnessKind) -> HarnessResult<Arc<dyn Harness>> {
        self.registry
            .read()
            .map_err(|_| HarnessError::config("harness registry lock poisoned"))?
            .get_kind(kind)
    }

    pub async fn execute(
        &self,
        descriptor: &TargetDescriptor,
        mut request: crate::models::AttackRequest,
    ) -> HarnessResult<crate::models::NormalizedResponse> {
        apply_purpose_policy(&mut request);
        let interceptors = self
            .interceptors
            .read()
            .map_err(|_| HarnessError::config("harness interceptor lock poisoned"))?
            .clone();
        for interceptor in &interceptors {
            match interceptor.pre_execute(&mut request).await? {
                InterceptAction::Continue => {}
                InterceptAction::Deny { reason } => return Err(HarnessError::Denied(reason)),
            }
        }
        let harness = self.resolve(descriptor)?;
        let mut attempts = 0u32;
        loop {
            attempts += 1;
            request.cancel.check()?;
            let result = harness.execute(request.clone()).await;
            match result {
                Ok(mut response) => {
                    for interceptor in &interceptors {
                        interceptor.post_execute(&request, &mut response).await?;
                    }
                    let mut response = self.normalizer.normalize(&request, response)?;
                    crate::redact::redact_response(&request, &mut response);
                    if should_retry_response(&request.purpose, &response)
                        && attempts < retry_response_limit(response.error_class.as_deref())
                    {
                        let wait_ms = retry_after_ms(&response)
                            .unwrap_or_else(|| backoff_ms(attempts));
                        tracing::info!(
                            purpose = request.purpose.as_str(),
                            attempt = attempts,
                            wait_ms,
                            error_class = response.error_class.as_deref().unwrap_or("http"),
                            "harness retry (not model-visible)"
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
                        continue;
                    }
                    if request.purpose.fails_on_retryable_http() {
                        if let Some(err) = inference_error_from_response(&response) {
                            request.emit_finish(None, Some(err.error_class().into()));
                            return Err(err);
                        }
                    }
                    cap_raw_response(&mut response);
                    response
                        .metadata
                        .insert("purpose".into(), request.purpose.as_str().to_string());
                    response
                        .metadata
                        .insert("attempts".into(), attempts.to_string());
                    if let (Some(input), Some(output)) =
                        (response.usage_input_tokens, response.usage_output_tokens)
                    {
                        request.emit_chunk(crate::models::StreamChunk::Usage {
                            input_tokens: input,
                            output_tokens: output,
                        });
                    }
                    request.emit_finish(response.stop_reason.clone(), response.error_class.clone());
                    return Ok(response);
                }
                Err(err) if matches!(err, HarnessError::Cancelled | HarnessError::Denied(_)) => {
                    request.emit_finish(None, Some(err.error_class().into()));
                    return Err(err);
                }
                Err(err) if err.is_retryable() && attempts < retry_error_limit(&err) => {
                    let wait_ms = match &err {
                        HarnessError::RateLimited { retry_after_ms } => {
                            retry_after_ms.unwrap_or_else(|| backoff_ms(attempts))
                        }
                        _ => backoff_ms(attempts),
                    };
                    tracing::info!(
                        purpose = request.purpose.as_str(),
                        attempt = attempts,
                        wait_ms,
                        error = %err,
                        "harness retry after transport error"
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
                }
                Err(err) => {
                    request.emit_finish(None, Some(err.error_class().into()));
                    return Err(err);
                }
            }
        }
    }
}

const MAX_ATTEMPTS: u32 = 4;
/// Timeouts already waited the full request timeout — one retry, not 3×121s pile-ups.
const TIMEOUT_ATTEMPTS: u32 = 2;
const MAX_RAW_BYTES: usize = 2 * 1024 * 1024;

fn retry_error_limit(err: &HarnessError) -> u32 {
    match err {
        HarnessError::Timeout(_) => TIMEOUT_ATTEMPTS,
        _ => MAX_ATTEMPTS,
    }
}

fn retry_response_limit(error_class: Option<&str>) -> u32 {
    match error_class {
        Some("timeout") => TIMEOUT_ATTEMPTS,
        _ => MAX_ATTEMPTS,
    }
}

fn apply_purpose_policy(request: &mut crate::models::AttackRequest) {
    match request.purpose.as_str() {
        "wizard" => {
            request.max_tokens = Some(request.max_tokens.unwrap_or(8192).min(8192));
        }
        "judge" | "health" => {
            if request.temperature.is_none() {
                request.temperature = Some(0.0);
            }
        }
        "report" => {
            request.max_tokens = Some(request.max_tokens.unwrap_or(4096).min(4096));
        }
        _ => {}
    }
}

fn should_retry_response(
    purpose: &crate::models::HarnessPurpose,
    response: &crate::models::NormalizedResponse,
) -> bool {
    match response.error_class.as_deref() {
        Some("rate_limit") | Some("timeout") => true,
        Some("empty") if purpose.is_product_inference() => true,
        _ => matches!(response.status_code, Some(429 | 502 | 503 | 408 | 504)),
    }
}

fn inference_error_from_response(
    response: &crate::models::NormalizedResponse,
) -> Option<HarnessError> {
    match response.error_class.as_deref() {
        Some("rate_limit") => Some(HarnessError::RateLimited {
            retry_after_ms: retry_after_ms(response),
        }),
        Some("timeout") => Some(HarnessError::Timeout(format!(
            "http {}",
            response.status_code.unwrap_or(0)
        ))),
        Some("empty") => Some(HarnessError::Empty),
        Some("auth") => Some(HarnessError::auth(
            crate::provider_error_detail(&response.raw_response)
                .filter(|text| !text.trim().is_empty())
                .unwrap_or_else(|| response.content.clone()),
        )),
        Some("http") => Some(http_status_error(response)),
        _ => match response.status_code {
            Some(429) => Some(HarnessError::RateLimited {
                retry_after_ms: retry_after_ms(response),
            }),
            Some(408 | 504) => Some(HarnessError::Timeout(format!(
                "http {}",
                response.status_code.unwrap_or(0)
            ))),
            Some(502 | 503) => Some(HarnessError::transport(format!(
                "http {}",
                response.status_code.unwrap_or(0)
            ))),
            Some(status) if status >= 400 => Some(http_status_error(response)),
            _ => None,
        },
    }
}

fn http_status_error(response: &crate::models::NormalizedResponse) -> HarnessError {
    let status = response.status_code.unwrap_or(0);
    let message = crate::provider_error_detail(&response.raw_response)
        .or_else(|| {
            let trimmed = response.content.trim();
            if trimmed.is_empty() {
                None
            } else if trimmed.len() > 400 {
                Some(format!("{}…", &trimmed[..400]))
            } else {
                Some(trimmed.to_string())
            }
        })
        .unwrap_or_else(|| format!("http {status}"));
    HarnessError::Http { status, message }
}

fn retry_after_ms(response: &crate::models::NormalizedResponse) -> Option<u64> {
    let header = response
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("retry-after"))
        .map(|(_, value)| value.trim())?;
    header.parse::<u64>().ok().map(|secs| secs.saturating_mul(1000))
}

fn backoff_ms(attempt: u32) -> u64 {
    250u64.saturating_mul(2u64.saturating_pow(attempt.saturating_sub(1))).min(8_000)
}

fn cap_raw_response(response: &mut crate::models::NormalizedResponse) {
    if response.raw_response.len() > MAX_RAW_BYTES {
        response.raw_response.truncate(MAX_RAW_BYTES);
        response
            .metadata
            .insert("truncated".into(), "true".into());
    }
}

impl Default for HarnessFactory {
    fn default() -> Self {
        Self::new().expect("harness factory")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TargetSurface;

    #[test]
    fn factory_delegates_to_registry() {
        let factory = HarnessFactory::new().unwrap();
        let ids = factory.registered_ids().unwrap();
        assert!(ids.contains(&"http".to_string()));
        assert!(ids.contains(&"openai".to_string()));
        assert!(ids.contains(&"anthropic".to_string()));
        assert!(ids.contains(&"mcp".to_string()));
        assert!(ids.contains(&"llama".to_string()));

        let descriptor = TargetDescriptor {
            url: "https://api.example.com/v1/chat".into(),
            surface: TargetSurface::OpenAiCompatible,
            ..TargetDescriptor::default()
        };
        let harness = factory.resolve(&descriptor).unwrap();
        assert_eq!(harness.id(), "openai");
    }

    #[test]
    fn anthropic_surface_resolves_anthropic_harness() {
        let factory = HarnessFactory::new().unwrap();
        let descriptor = TargetDescriptor {
            url: "https://api.anthropic.com/v1/messages".into(),
            surface: TargetSurface::AnthropicCompatible,
            ..TargetDescriptor::default()
        };
        assert_eq!(factory.resolve(&descriptor).unwrap().id(), "anthropic");
    }

    #[test]
    fn isolated_registry_does_not_leak_to_parent() {
        struct Dummy;
        #[async_trait::async_trait]
        impl Harness for Dummy {
            fn id(&self) -> &'static str {
                "dummy"
            }
            async fn execute(
                &self,
                _request: crate::models::AttackRequest,
            ) -> HarnessResult<crate::models::NormalizedResponse> {
                Ok(crate::models::NormalizedResponse::from_http(
                    200,
                    "ok".into(),
                    "dummy",
                ))
            }
        }

        let base = HarnessFactory::new().unwrap();
        let isolated = base.isolated();
        isolated.register(Arc::new(Dummy)).unwrap();
        assert!(isolated
            .registered_ids()
            .unwrap()
            .contains(&"dummy".to_string()));
        assert!(!base
            .registered_ids()
            .unwrap()
            .contains(&"dummy".to_string()));
    }

    #[test]
    fn exhausted_429_is_rate_limited_error() {
        use crate::models::HarnessPurpose;

        let response = crate::models::NormalizedResponse::from_http(
            429,
            r#"{"status":429,"title":"Too Many Requests"}"#.into(),
            "openai",
        );
        let err = inference_error_from_response(&response).expect("429 maps to error");
        assert!(matches!(err, HarnessError::RateLimited { .. }));
        assert!(!HarnessPurpose::attack().fails_on_retryable_http());
        assert!(HarnessPurpose::assistant().fails_on_retryable_http());
    }

    #[test]
    fn gone_410_is_http_error_with_provider_detail() {
        let response = crate::models::NormalizedResponse::from_http(
            410,
            r#"{"type":"about:blank","title":"Gone","status":410,"detail":"The model 'meta/llama-3.1-8b-instruct' has reached its end of life on 2026-08-26T09:00:00Z and is no longer available."}"#.into(),
            "openai",
        );
        let err = inference_error_from_response(&response).expect("410 maps to error");
        match err {
            HarnessError::Http { status, message } => {
                assert_eq!(status, 410);
                assert!(message.contains("end of life"), "{message}");
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn timeout_retries_once_not_three_times() {
        assert_eq!(
            retry_error_limit(&HarnessError::Timeout("slow".into())),
            2
        );
        assert_eq!(retry_response_limit(Some("timeout")), 2);
        assert_eq!(
            retry_error_limit(&HarnessError::Transport("reset".into())),
            4
        );
        assert_eq!(retry_response_limit(Some("rate_limit")), 4);
    }
}
