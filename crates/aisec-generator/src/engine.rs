use aisec_planner::AttackPlan;

use crate::error::GeneratorResult;
use crate::local_llm::{generate_local_llm, GeneratorLlm};
use crate::static_pack::generate_static_pack;
use crate::template_mutation::generate_template_mutation;
use crate::types::{GeneratePayloadsInput, GeneratorMode, PromptPayloads};

/// Generate prompt payloads from a planner attack plan.
pub async fn generate_prompt_payloads(
    input: &GeneratePayloadsInput<'_>,
) -> GeneratorResult<PromptPayloads> {
    generate_prompt_payloads_with_llm(input, None).await
}

/// Generate prompt payloads, optionally using a local LLM backend.
pub async fn generate_prompt_payloads_with_llm(
    input: &GeneratePayloadsInput<'_>,
    llm: Option<&dyn GeneratorLlm>,
) -> GeneratorResult<PromptPayloads> {
    match input.mode {
        GeneratorMode::StaticPack => generate_static_pack(input),
        GeneratorMode::TemplateMutation => generate_template_mutation(input.plan, input),
        GeneratorMode::LocalLlm => {
            let backend = llm.ok_or_else(|| {
                crate::error::GeneratorError::InvalidInput(
                    "local LLM generator requires a configured vault model".into(),
                )
            })?;
            generate_local_llm(input.plan, input, backend).await
        }
    }
}

/// Convenience wrapper accepting an attack plan directly.
pub async fn generate_from_plan(
    plan: &AttackPlan,
    mode: GeneratorMode,
    llm: Option<&dyn GeneratorLlm>,
) -> GeneratorResult<PromptPayloads> {
    let input = GeneratePayloadsInput::new(plan, mode);
    generate_prompt_payloads_with_llm(&input, llm).await
}
