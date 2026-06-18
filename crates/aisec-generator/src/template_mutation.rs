use std::collections::HashMap;

use aisec_attack::{AttackCategory, AttackPayload};
use aisec_payload::{GenerateRequest, MutationKind, PayloadPipeline};
use aisec_planner::AttackPlan;

use crate::convert::{attack_to_payload_category, generated_to_attack_payload};
use crate::error::GeneratorResult;
use crate::static_pack::finish_pack;
use crate::types::{GeneratePayloadsInput, GeneratorMode, PromptPayloads};

pub fn generate_template_mutation(
    plan: &AttackPlan,
    input: &GeneratePayloadsInput<'_>,
) -> GeneratorResult<PromptPayloads> {
    let pipeline = PayloadPipeline::with_defaults()?;
    let disabled: std::collections::HashSet<_> = plan.disabled_tests.iter().cloned().collect();
    let max_per = input.max_variants_per_payload.unwrap_or(4);
    let mut by_category: HashMap<AttackCategory, Vec<AttackPayload>> = HashMap::new();
    let mut source_count = 0usize;
    let mut variant_count = 0usize;

    for category in &plan.categories {
        let payload_cat = attack_to_payload_category(*category);
        let report = pipeline.generate(&GenerateRequest {
            categories: Some(vec![payload_cat]),
            mutations: MutationKind::encoding_kinds().to_vec(),
            max_variants_per_payload: Some(max_per),
            ..Default::default()
        })?;

        let payloads: Vec<AttackPayload> = report
            .variants
            .iter()
            .filter(|v| !disabled.contains(&v.source_id))
            .map(generated_to_attack_payload)
            .collect();

        source_count += report.stats.source_count;
        variant_count += payloads.len();
        if payloads.is_empty() {
            continue;
        }
        by_category.insert(*category, payloads);
    }

    let mut pack = finish_pack(
        GeneratorMode::TemplateMutation,
        plan,
        by_category,
        source_count,
        None,
    );
    pack.stats.variant_count = variant_count;
    pack.summary = format!(
        "template mutation · {} categories · {} variants (profile {})",
        pack.stats.category_count, variant_count, plan.profile_id
    );
    Ok(pack)
}
