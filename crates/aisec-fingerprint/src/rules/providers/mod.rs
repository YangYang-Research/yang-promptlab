//! Provider-specific fingerprint rules.

mod anthropic;
mod azure_openai;
mod bedrock;
mod gemini;
mod litellm;
mod ollama;
mod openai;
mod openrouter;
mod vllm;

use super::FingerprintRule;

pub fn all_rules() -> Vec<FingerprintRule> {
    let mut rules = Vec::new();
    rules.extend(openai::rules());
    rules.extend(anthropic::rules());
    rules.extend(gemini::rules());
    rules.extend(bedrock::rules());
    rules.extend(azure_openai::rules());
    rules.extend(ollama::rules());
    rules.extend(litellm::rules());
    rules.extend(vllm::rules());
    rules.extend(openrouter::rules());
    rules
}
