use std::sync::Arc;

use async_trait::async_trait;
use aisec_models::runtime::InferenceRuntime;
use aisec_models::types::InferenceRequest;
use tokio::sync::Mutex;

use crate::error::{JudgeError, JudgeResult};
use crate::prompts::RolePrompts;
use crate::types::{EvaluatorKind, EvaluatorResult, JudgeRequest, ModelRole, Severity};

/// Parses JSON LLM judge responses.
pub struct LlmResponseParser;

impl LlmResponseParser {
    pub fn parse(text: &str) -> JudgeResult<ParsedLlmVerdict> {
        let json_str = extract_json_object(text);
        let value: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| JudgeError::evaluation(format!("invalid LLM JSON: {e}")))?;

        let vulnerable = value
            .get("vulnerable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let confidence = value
            .get("confidence")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32)
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);
        let severity = value
            .get("severity")
            .and_then(|v| v.as_str())
            .and_then(Severity::from_str_loose);
        let category = value
            .get("category")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let rationale = value
            .get("rationale")
            .and_then(|v| v.as_str())
            .unwrap_or("LLM evaluation")
            .to_string();
        let indicators = value
            .get("indicators")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        Ok(ParsedLlmVerdict {
            vulnerable,
            confidence,
            severity,
            category,
            rationale,
            indicators,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ParsedLlmVerdict {
    pub vulnerable: bool,
    pub confidence: f32,
    pub severity: Option<Severity>,
    pub category: Option<String>,
    pub rationale: String,
    pub indicators: Vec<String>,
}

fn extract_json_object(text: &str) -> String {
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end >= start {
                return text[start..=end].to_string();
            }
        }
    }
    text.trim().to_string()
}

/// LLM evaluator backed by offline llama.cpp runtime.
pub struct LlmEvaluator {
    id: String,
    role: ModelRole,
    runtime: Arc<Mutex<dyn InferenceRuntime>>,
    max_tokens: u32,
    temperature: f32,
}

impl LlmEvaluator {
    pub fn new(
        role: ModelRole,
        runtime: Arc<Mutex<dyn InferenceRuntime>>,
        max_tokens: u32,
        temperature: f32,
    ) -> Self {
        Self {
            id: format!("llm-{}", role.as_str()),
            role,
            runtime,
            max_tokens,
            temperature,
        }
    }

    pub fn role(&self) -> ModelRole {
        self.role
    }

    fn build_prompt(&self, request: &JudgeRequest) -> String {
        format!(
            "{system}\n\n{user}",
            system = RolePrompts::system(self.role),
            user = RolePrompts::user(
                self.role,
                &request.attack_category,
                &request.payload,
                &request.response_text,
            ),
        )
    }

    pub async fn evaluate_async(&self, request: &JudgeRequest) -> JudgeResult<EvaluatorResult> {
        let prompt = self.build_prompt(request);
        let runtime = self.runtime.lock().await;

        let response = runtime
            .complete(InferenceRequest {
                prompt,
                max_tokens: self.max_tokens,
                temperature: self.temperature,
            })
            .await
            .map_err(|e| JudgeError::evaluation(e.to_string()))?;

        let parsed = LlmResponseParser::parse(&response.text)?;

        Ok(EvaluatorResult {
            evaluator_id: self.id.clone(),
            kind: EvaluatorKind::Llm,
            role: Some(self.role),
            vulnerable: parsed.vulnerable,
            confidence: parsed.confidence,
            severity: parsed.severity,
            category: parsed.category,
            rationale: parsed.rationale,
            indicators: parsed.indicators,
        })
    }
}

#[async_trait]
impl crate::evaluators::Evaluator for LlmEvaluator {
    fn id(&self) -> &str {
        &self.id
    }

    async fn evaluate(&self, request: &JudgeRequest) -> JudgeResult<EvaluatorResult> {
        self.evaluate_async(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_llm_json() {
        let parsed = LlmResponseParser::parse(
            r#"Here is my analysis: {"vulnerable": true, "confidence": 0.91, "severity": "high", "rationale": "leaked key", "indicators": ["secret"]}"#,
        )
        .unwrap();
        assert!(parsed.vulnerable);
        assert!(parsed.confidence > 0.9);
    }

    #[test]
    fn extracts_embedded_json() {
        let parsed = LlmResponseParser::parse("```json\n{\"vulnerable\": false, \"confidence\": 0.2}\n```").unwrap();
        assert!(!parsed.vulnerable);
    }
}
