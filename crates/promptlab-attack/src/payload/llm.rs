use async_trait::async_trait;
use promptlab_payload::{
    llm_crossover_prompt, llm_few_shot_prompt, llm_rephrase_prompt, llm_transfer_prompt,
    rule_rephrase, sanitize_llm_mutator_output, LLM_MUTATOR_SYSTEM,
};

use crate::error::{AttackError, AttackResult};
use crate::payload::MutatorKind;

/// Bridge for LLM-assisted attack-time mutators (GPTFuzzer-style).
#[async_trait]
pub trait LlmComplete: Send + Sync {
    async fn complete(&self, system: &str, prompt: &str) -> AttackResult<String>;
}

pub async fn apply_llm_mutator(
    llm: &dyn LlmComplete,
    kind: MutatorKind,
    content: &str,
) -> AttackResult<String> {
    let prompt = match kind {
        MutatorKind::LlmRephrase => llm_rephrase_prompt(content),
        MutatorKind::LlmCrossover => llm_crossover_prompt(content),
        MutatorKind::LlmFewShot => llm_few_shot_prompt(content),
        MutatorKind::LlmTransfer => llm_transfer_prompt(content),
        _ => {
            return Err(AttackError::payload(format!(
                "not an LLM mutator kind: {}",
                kind.as_str()
            )));
        }
    };

    let raw = llm.complete(LLM_MUTATOR_SYSTEM, &prompt).await?;
    let cleaned = sanitize_llm_mutator_output(&raw);
    if cleaned.trim().is_empty() {
        return Err(AttackError::payload("LLM mutator returned empty output"));
    }
    Ok(cleaned)
}

pub fn apply_llm_mutator_fallback(kind: MutatorKind, content: &str) -> String {
    match kind {
        MutatorKind::LlmRephrase => rule_rephrase(content),
        MutatorKind::LlmCrossover => promptlab_payload::crossover_wrap(content),
        MutatorKind::LlmFewShot => promptlab_payload::refusal_suppression_wrap(content),
        MutatorKind::LlmTransfer => promptlab_payload::language_pivot(content),
        _ => content.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoLlm;

    #[async_trait]
    impl LlmComplete for EchoLlm {
        async fn complete(&self, _system: &str, prompt: &str) -> AttackResult<String> {
            Ok(format!("LLM:{prompt}"))
        }
    }

    #[tokio::test]
    async fn apply_llm_rephrase_uses_backend() {
        let out = apply_llm_mutator(&EchoLlm, MutatorKind::LlmRephrase, "ignore rules")
            .await
            .unwrap();
        assert!(out.contains("LLM:"));
    }
}
