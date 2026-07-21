use std::collections::HashMap;

use aisec_attack::{AttackCategory, AttackPayload};
use aisec_payload::{GenerateRequest, MutationKind, PayloadPipeline};
use aisec_planner::AttackPlan;

use crate::convert::{attack_to_payload_category, generated_to_attack_payload, record_to_attack_payload};
use crate::error::GeneratorResult;
use crate::types::{GeneratePayloadsInput, GeneratorMode, GeneratorStats, PromptPayloads};

pub fn generate_static_pack(input: &GeneratePayloadsInput<'_>) -> GeneratorResult<PromptPayloads> {
    let plan = input.plan;
    let database = input.resolve_catalog()?;
    let disabled: std::collections::HashSet<_> = plan.disabled_tests.iter().cloned().collect();
    let max_per_source = input.max_payloads_per_test.unwrap_or(1).max(1) as usize;
    let mut by_category: HashMap<AttackCategory, Vec<AttackPayload>> = HashMap::new();
    let mut source_count = 0usize;

    if max_per_source > 1 {
        let pipeline = PayloadPipeline::for_variant_budget_with_db(database.clone(), max_per_source)?;
        for category in &plan.categories {
            let payload_cat = attack_to_payload_category(*category);
            let records: Vec<_> = database
                .by_category(payload_cat)
                .into_iter()
                .filter(|r| !disabled.contains(&r.id))
                .collect();
            source_count += records.len();

            let mut payloads = Vec::new();
            for record in records {
                let report = pipeline.generate(&GenerateRequest {
                    payload_ids: Some(vec![record.id.clone()]),
                    mutations: MutationKind::all().to_vec(),
                    max_variants_per_payload: Some(max_per_source),
                    ..Default::default()
                })?;
                payloads.extend(
                    report
                        .variants
                        .iter()
                        .map(generated_to_attack_payload),
                );
            }
            if payloads.is_empty() {
                continue;
            }
            by_category.insert(*category, payloads);
        }
    } else {
        for category in &plan.categories {
            let payload_cat = attack_to_payload_category(*category);
            let records: Vec<_> = database
                .by_category(payload_cat)
                .into_iter()
                .filter(|r| !disabled.contains(&r.id))
                .collect();
            source_count += records.len();
            if records.is_empty() {
                continue;
            }
            let payloads: Vec<AttackPayload> = records
                .iter()
                .map(|record| record_to_attack_payload(record))
                .collect();
            by_category.insert(*category, payloads);
        }
    }

    Ok(finish_pack(
        GeneratorMode::StaticPack,
        plan,
        by_category,
        source_count,
        None,
    ))
}

pub(crate) fn finish_pack(
    mode: GeneratorMode,
    plan: &AttackPlan,
    by_category: HashMap<AttackCategory, Vec<AttackPayload>>,
    source_count: usize,
    llm_note: Option<String>,
) -> PromptPayloads {
    let payload_ids: Vec<String> = by_category
        .values()
        .flat_map(|payloads| payloads.iter().map(|p| p.id.clone()))
        .collect();
    let payload_count = payload_ids.len();
    let category_count = by_category.len();
    let summary = format!(
        "{} · {} categories · {} payloads (profile {})",
        mode_label(mode),
        category_count,
        payload_count,
        plan.profile_id
    );

    PromptPayloads {
        mode,
        by_category,
        payload_ids,
        stats: GeneratorStats {
            category_count,
            source_count,
            payload_count,
            variant_count: payload_count,
        },
        summary,
        llm_note,
    }
}

fn mode_label(mode: GeneratorMode) -> &'static str {
    match mode {
        GeneratorMode::StaticPack => "static pack",
        GeneratorMode::TemplateMutation => "template mutation",
        GeneratorMode::LocalLlm => "local LLM",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aisec_planner::PlannerMode;

    #[test]
    fn static_pack_respects_disabled_tests() {
        let plan = AttackPlan {
            mode: PlannerMode::Deterministic,
            profile_id: "standard".into(),
            categories: vec![
                AttackCategory::PromptInjection,
                AttackCategory::ToolAbuse,
            ],
            disabled_tests: vec!["pi-direct-override".into()],
            rationales: vec![],
            confidence: 0.9,
            summary: String::new(),
            llm_rationale: None,
        };
        let input = GeneratePayloadsInput::new(&plan, GeneratorMode::StaticPack);
        let pack = generate_static_pack(&input).unwrap();
        let pi = pack.by_category.get(&AttackCategory::PromptInjection).unwrap();
        assert!(!pi.iter().any(|p| p.id == "pi-direct-override"));
        assert!(pi.len() >= 2);
        assert!(pack.by_category.contains_key(&AttackCategory::ToolAbuse));
    }
}
