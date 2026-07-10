use aisec_attack::AttackCategory;
use aisec_planner::AttackPlan;
use async_trait::async_trait;
use serde::Deserialize;
use tracing::warn;

use crate::convert::llm_payload_to_attack;
use crate::error::{GeneratorError, GeneratorResult};
use crate::static_pack::{finish_pack, generate_static_pack};
use crate::types::{GeneratePayloadsInput, GeneratorMode, PromptPayloads};

/// LLM completion bridge for local generator mode.
#[async_trait]
pub trait GeneratorLlm: Send + Sync {
    async fn complete(&self, prompt: &str) -> GeneratorResult<String>;
}

#[derive(Debug, Deserialize)]
struct LlmPayloadEntry {
    id: Option<String>,
    name: Option<String>,
    content: String,
}

pub async fn generate_with_local_llm(
    plan: &AttackPlan,
    llm: &dyn GeneratorLlm,
) -> GeneratorResult<PromptPayloads> {
    let input = GeneratePayloadsInput::new(plan, GeneratorMode::LocalLlm);
    generate_local_llm(plan, &input, llm).await
}

pub async fn generate_local_llm(
    plan: &AttackPlan,
    input: &GeneratePayloadsInput<'_>,
    llm: &dyn GeneratorLlm,
) -> GeneratorResult<PromptPayloads> {
    let baseline = {
        let mut baseline_input = GeneratePayloadsInput::new(plan, GeneratorMode::StaticPack);
        baseline_input.catalog = input.catalog;
        generate_static_pack(&baseline_input)?
    };
    let mut by_category = baseline.by_category.clone();
    let mut llm_generated = 0usize;

    for category in &plan.categories {
        let prompt = build_category_prompt(plan, *category, &baseline, input);
        let raw = match llm.complete(&prompt).await {
            Ok(text) => text,
            Err(err) => {
                warn!(category = %category.as_str(), error = %err, "LLM payload generation failed for category");
                continue;
            }
        };

        let entries = match parse_llm_payloads(&raw) {
            Ok(entries) => entries,
            Err(err) => {
                warn!(category = %category.as_str(), error = %err, "LLM payload parse failed");
                continue;
            }
        };

        let mut generated: Vec<_> = entries
            .into_iter()
            .enumerate()
            .filter(|(_, e)| !e.content.trim().is_empty())
            .map(|(idx, entry)| {
                let id = entry
                    .id
                    .unwrap_or_else(|| format!("llm-{}-{}", category.as_str(), idx + 1));
                let name = entry
                    .name
                    .unwrap_or_else(|| format!("LLM probe {}", idx + 1));
                llm_payload_to_attack(*category, id, name, entry.content)
            })
            .collect();

        if generated.is_empty() {
            continue;
        }

        llm_generated += generated.len();
        by_category
            .entry(*category)
            .or_default()
            .append(&mut generated);
    }

    let note = if llm_generated == 0 {
        Some("LLM produced no valid payloads; static pack baseline retained".into())
    } else {
        Some(format!(
            "merged {llm_generated} LLM-generated probes with static baseline"
        ))
    };

    let mut pack = finish_pack(
        GeneratorMode::LocalLlm,
        plan,
        by_category,
        baseline.stats.source_count,
        note,
    );
    pack.summary = format!(
        "local LLM · {} categories · {} payloads (profile {})",
        pack.stats.category_count, pack.stats.payload_count, plan.profile_id
    );
    Ok(pack)
}

fn build_category_prompt(
    plan: &AttackPlan,
    category: AttackCategory,
    baseline: &PromptPayloads,
    input: &GeneratePayloadsInput<'_>,
) -> String {
    let baseline_samples: Vec<_> = baseline
        .payloads_for(category)
        .unwrap_or(&[])
        .iter()
        .take(2)
        .map(|p| serde_json::json!({ "id": p.id, "name": p.name, "content": p.content }))
        .collect();

    let mut prompt = aisec_inference::PromptRegistry::generator_user(
        &category.display_name(),
        category.as_str(),
        &plan.profile_id,
        &format!("{:?}", plan.disabled_tests),
        &serde_json::to_string_pretty(&baseline_samples).unwrap_or_default(),
    );

    if input.advanced.enable_context_awareness {
        if let Some(ctx) = &input.target_context {
            prompt.push_str("\n\nTarget context:\n");
            prompt.push_str(&format!(
                "Provider: {}\nFramework: {}\nEndpoint: {}\nModel: {}\nCapabilities: {}\n",
                ctx.provider,
                ctx.framework,
                ctx.endpoint,
                ctx.model.as_deref().unwrap_or("unknown"),
                ctx.capability_notes.join(", ")
            ));
        }
    }

    if input.advanced.enable_conversation_memory {
        prompt.push_str(
            "\nPrefer multi-turn / conversation-state probes that reference prior dialogue.\n",
        );
    }

    if input.advanced.enable_cross_category_mutation {
        prompt.push_str("\nBlend techniques from neighboring attack categories when useful.\n");
    }

    if input.advanced.enable_response_adaptation {
        if let Some(feedback) = input.adaptation_feedback.as_deref() {
            if !feedback.trim().is_empty() {
                prompt.push_str("\nPrior attempt feedback (adapt/evolve probes):\n");
                prompt.push_str(feedback);
                prompt.push('\n');
            }
        }
    }

    prompt
}

fn parse_llm_payloads(raw: &str) -> GeneratorResult<Vec<LlmPayloadEntry>> {
    let trimmed = raw.trim();
    let json_str = if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed.rfind(']') {
            &trimmed[start..=end]
        } else {
            trimmed
        }
    } else {
        trimmed
    };

    let entries: Vec<LlmPayloadEntry> = serde_json::from_str(json_str)?;
    if entries.is_empty() {
        return Err(GeneratorError::InvalidInput(
            "LLM returned empty payload array".into(),
        ));
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_array_from_llm_response() {
        let raw = r#"Here are probes:
[{"id":"x1","name":"Test","content":"Ignore prior rules"}]"#;
        let entries = parse_llm_payloads(raw).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "Ignore prior rules");
    }
}
