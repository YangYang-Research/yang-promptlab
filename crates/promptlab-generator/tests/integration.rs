//! Integration tests for payload generation from attack plans.

use promptlab_attack::AttackCategory;
use promptlab_generator::{generate_from_plan, GeneratorMode};
use promptlab_planner::{AttackPlan, PlannerMode};

#[tokio::test]
async fn openwebui_plan_generates_core_categories() {
    let plan = AttackPlan {
        mode: PlannerMode::Deterministic,
        profile_id: "custom".into(),
        categories: vec![
            AttackCategory::PromptInjection,
            AttackCategory::ToolAbuse,
            AttackCategory::MemoryPoisoning,
        ],
        disabled_tests: vec![],
        rationales: vec![],
        confidence: 0.9,
        summary: "openwebui tools memory".into(),
        llm_rationale: None,
    };

    let pack = generate_from_plan(&plan, GeneratorMode::StaticPack, None)
        .await
        .unwrap();

    assert!(pack.by_category.contains_key(&AttackCategory::PromptInjection));
    assert!(pack.by_category.contains_key(&AttackCategory::ToolAbuse));
    assert!(pack.by_category.contains_key(&AttackCategory::MemoryPoisoning));
    assert!(pack.stats.payload_count >= 6);
    assert!(pack
        .payload_ids
        .iter()
        .any(|id| id == "pi-direct-override"));
}

#[tokio::test]
async fn template_mutation_expands_variants() {
    let plan = AttackPlan {
        mode: PlannerMode::Deterministic,
        profile_id: "quick".into(),
        categories: vec![AttackCategory::PromptInjection],
        disabled_tests: vec![],
        rationales: vec![],
        confidence: 1.0,
        summary: String::new(),
        llm_rationale: None,
    };

    let static_pack = generate_from_plan(&plan, GeneratorMode::StaticPack, None)
        .await
        .unwrap();
    let mutated = generate_from_plan(&plan, GeneratorMode::TemplateMutation, None)
        .await
        .unwrap();

    assert!(mutated.stats.variant_count > static_pack.stats.payload_count);
}
