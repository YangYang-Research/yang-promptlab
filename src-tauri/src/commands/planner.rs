//! Attack planner IPC — wizard attack plan DTOs and adjust command.

use promptlab_attack::AttackCategory;
use promptlab_storage::TargetRepository;
use promptlab_target_profile::{
    adjust_wizard_attack_plan, build_wizard_attack_plan, build_wizard_plan_summary,
    AttackProfileMode, ExecutionStrategy, PayloadStrategy, WizardAttackPlan,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttackGraphNodeDto {
    pub category: String,
    pub priority: u8,
    pub risk: u8,
    pub confidence: f32,
    pub dependencies: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttackProfileModeDto {
    pub profile_id: String,
    #[serde(default)]
    pub description: String,
    pub categories: Vec<String>,
    pub execution_strategy: String,
    pub max_attempts: u8,
    pub reflection_enabled: bool,
    pub adaptive_planning: bool,
    pub payload_strategy: PayloadStrategyDto,
    #[serde(default)]
    pub disabled_tests: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WizardAttackPlanDto {
    pub profile_id: String,
    pub recommended_profile_id: String,
    pub suggested_categories: Vec<String>,
    pub profile_modes: Vec<AttackProfileModeDto>,
    pub categories: Vec<String>,
    pub disabled_tests: Vec<String>,
    pub capability_graph: Vec<String>,
    pub attack_graph: Vec<AttackGraphNodeDto>,
    pub execution_strategy: String,
    pub max_attempts: u8,
    pub reflection_enabled: bool,
    pub adaptive_planning: bool,
    pub rationales: Vec<CategoryRationaleDto>,
    pub confidence: f32,
    pub summary: String,
    pub risk_score: u8,
    pub risk_level: String,
    pub estimated_requests: u32,
    pub estimated_runtime_seconds: u32,
    pub estimated_tokens: u32,
    pub coverage_score: f32,
    pub risk_coverage: f32,
    pub total_testcases: u32,
    pub payload_strategy: PayloadStrategyDto,
    pub         recommended_payload_strategy: PayloadStrategyDto,
    pub planner_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadStrategyDto {
    pub strategy: String,
    pub mutation_level: String,
    pub variants_per_test: u32,
    pub max_total_payloads: u32,
    pub enable_context_awareness: bool,
    pub enable_conversation_memory: bool,
    pub enable_response_adaptation: bool,
    pub enable_payload_deduplication: bool,
    pub enable_cross_category_mutation: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannerAdjustRequest {
    pub target_id: String,
    pub profile_id: String,
    pub categories: Vec<String>,
    pub disabled_tests: Vec<String>,
    pub disabled_graph_nodes: Vec<String>,
    pub execution_strategy: Option<String>,
    pub max_attempts: Option<u8>,
    pub reflection_enabled: Option<bool>,
    pub adaptive_planning: Option<bool>,
    pub payload_strategy: Option<PayloadStrategyDto>,
    #[serde(default)]
    pub suggested_categories: Vec<String>,
    #[serde(default)]
    pub profile_modes: Vec<AttackProfileModeDto>,
    #[serde(default)]
    pub rationales: Vec<CategoryRationaleDto>,
    #[serde(default)]
    pub capability_graph: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRationaleDto {
    pub category: String,
    pub reason: String,
    pub priority: u8,
    pub source: String,
}

fn payload_strategy_to_dto(strategy: PayloadStrategy) -> PayloadStrategyDto {
    PayloadStrategyDto {
        strategy: match strategy.strategy {
            promptlab_target_profile::PayloadGenerationStrategy::Deterministic => {
                "deterministic".into()
            }
            promptlab_target_profile::PayloadGenerationStrategy::Mutation => "mutation".into(),
            promptlab_target_profile::PayloadGenerationStrategy::Adaptive => "adaptive".into(),
        },
        mutation_level: match strategy.mutation_level {
            promptlab_target_profile::MutationLevel::Low => "low".into(),
            promptlab_target_profile::MutationLevel::Medium => "medium".into(),
            promptlab_target_profile::MutationLevel::High => "high".into(),
            promptlab_target_profile::MutationLevel::Extreme => "extreme".into(),
        },
        variants_per_test: strategy.variants_per_test,
        max_total_payloads: strategy.max_total_payloads,
        enable_context_awareness: strategy.enable_context_awareness,
        enable_conversation_memory: strategy.enable_conversation_memory,
        enable_response_adaptation: strategy.enable_response_adaptation,
        enable_payload_deduplication: strategy.enable_payload_deduplication,
        enable_cross_category_mutation: strategy.enable_cross_category_mutation,
    }
}

fn parse_payload_strategy(dto: PayloadStrategyDto) -> CommandResult<PayloadStrategy> {
    use promptlab_target_profile::{MutationLevel, PayloadGenerationStrategy};

    let strategy = match dto.strategy.trim().to_ascii_lowercase().as_str() {
        "deterministic" => PayloadGenerationStrategy::Deterministic,
        "adaptive" => PayloadGenerationStrategy::Adaptive,
        _ => PayloadGenerationStrategy::Mutation,
    };
    let mutation_level = match dto.mutation_level.trim().to_ascii_lowercase().as_str() {
        "low" => MutationLevel::Low,
        "high" => MutationLevel::High,
        "extreme" => MutationLevel::Extreme,
        _ => MutationLevel::Medium,
    };
    Ok(PayloadStrategy {
        strategy,
        mutation_level,
        variants_per_test: dto.variants_per_test,
        max_total_payloads: dto.max_total_payloads,
        enable_context_awareness: dto.enable_context_awareness,
        enable_conversation_memory: dto.enable_conversation_memory,
        enable_response_adaptation: dto.enable_response_adaptation,
        enable_payload_deduplication: dto.enable_payload_deduplication,
        enable_cross_category_mutation: dto.enable_cross_category_mutation,
    }
    .clamp())
}

pub fn wizard_plan_to_dto(plan: WizardAttackPlan) -> WizardAttackPlanDto {
    WizardAttackPlanDto {
        profile_id: plan.profile_id,
        recommended_profile_id: plan.recommended_profile_id,
        suggested_categories: plan
            .suggested_categories
            .iter()
            .map(|c| c.as_str().into())
            .collect(),
        profile_modes: plan
            .profile_modes
            .into_iter()
            .map(profile_mode_to_dto)
            .collect(),
        categories: plan.categories.iter().map(|c| c.as_str().into()).collect(),
        disabled_tests: plan.disabled_tests,
        capability_graph: plan.capability_graph,
        attack_graph: plan
            .attack_graph
            .into_iter()
            .map(|node| AttackGraphNodeDto {
                category: node.category.as_str().into(),
                priority: node.priority,
                risk: node.risk,
                confidence: node.confidence,
                dependencies: node
                    .dependencies
                    .iter()
                    .map(|c| c.as_str().into())
                    .collect(),
                enabled: node.enabled,
            })
            .collect(),
        execution_strategy: match plan.execution_strategy {
            ExecutionStrategy::Sequential => "sequential".into(),
            ExecutionStrategy::Agentic => "agentic".into(),
        },
        max_attempts: plan.max_attempts,
        reflection_enabled: plan.reflection_enabled,
        adaptive_planning: plan.adaptive_planning,
        rationales: plan
            .rationales
            .into_iter()
            .map(|r| CategoryRationaleDto {
                category: r.category.as_str().into(),
                reason: r.reason,
                priority: r.priority,
                source: r.source,
            })
            .collect(),
        confidence: plan.confidence,
        summary: plan.summary,
        risk_score: plan.risk_score,
        risk_level: plan.risk_level,
        estimated_requests: plan.estimated_requests,
        estimated_runtime_seconds: plan.estimated_runtime_seconds,
        estimated_tokens: plan.estimated_tokens,
        coverage_score: plan.coverage_score,
        risk_coverage: plan.risk_coverage,
        total_testcases: plan.total_testcases,
        payload_strategy: payload_strategy_to_dto(plan.payload_strategy),
        recommended_payload_strategy: payload_strategy_to_dto(plan.recommended_payload_strategy),
        planner_source: plan.planner_source,
    }
}

fn profile_mode_to_dto(mode: AttackProfileMode) -> AttackProfileModeDto {
    AttackProfileModeDto {
        profile_id: mode.profile_id,
        description: mode.description,
        categories: mode.categories.iter().map(|c| c.as_str().into()).collect(),
        execution_strategy: match mode.execution_strategy {
            ExecutionStrategy::Sequential => "sequential".into(),
            ExecutionStrategy::Agentic => "agentic".into(),
        },
        max_attempts: mode.max_attempts,
        reflection_enabled: mode.reflection_enabled,
        adaptive_planning: mode.adaptive_planning,
        payload_strategy: payload_strategy_to_dto(mode.payload_strategy),
        disabled_tests: mode.disabled_tests,
    }
}

fn parse_profile_modes(modes: &[AttackProfileModeDto]) -> Vec<AttackProfileMode> {
    modes
        .iter()
        .filter_map(|mode| {
            let categories = parse_attack_categories(&mode.categories);
            if categories.is_empty() {
                return None;
            }
            Some(AttackProfileMode {
                profile_id: mode.profile_id.clone(),
                description: mode.description.clone(),
                categories,
                execution_strategy: parse_execution_strategy(Some(&mode.execution_strategy)),
                max_attempts: mode.max_attempts.clamp(1, 20),
                reflection_enabled: mode.reflection_enabled,
                adaptive_planning: mode.adaptive_planning,
                payload_strategy: parse_payload_strategy(mode.payload_strategy.clone()).ok()?,
                disabled_tests: mode.disabled_tests.clone(),
            })
        })
        .collect()
}

fn parse_category_rationales(
    raw: &[CategoryRationaleDto],
) -> Vec<promptlab_planner::types::CategoryRationale> {
    raw.iter()
        .filter_map(|item| {
            let category = AttackCategory::all()
                .iter()
                .copied()
                .find(|c| c.as_str() == item.category)?;
            Some(promptlab_planner::types::CategoryRationale {
                category,
                reason: item.reason.clone(),
                priority: item.priority,
                source: item.source.clone(),
            })
        })
        .collect()
}

fn merge_adjust_base(profile: &promptlab_target_profile::TargetProfile, request: &PlannerAdjustRequest) -> WizardAttackPlan {
    let mut base = build_wizard_attack_plan(profile);
    if request.profile_modes.is_empty() {
        return base;
    }

    base.suggested_categories = if request.suggested_categories.is_empty() {
        base.suggested_categories
    } else {
        parse_attack_categories(&request.suggested_categories)
    };
    base.profile_modes = parse_profile_modes(&request.profile_modes);
    if !request.rationales.is_empty() {
        base.rationales = parse_category_rationales(&request.rationales);
    }
    if !request.capability_graph.is_empty() {
        base.capability_graph = request.capability_graph.clone();
    }
    base
}

fn parse_attack_categories(raw: &[String]) -> Vec<AttackCategory> {
    raw.iter()
        .filter_map(|value| AttackCategory::all().iter().copied().find(|c| c.as_str() == value))
        .collect()
}

fn parse_execution_strategy(raw: Option<&str>) -> ExecutionStrategy {
    match raw.unwrap_or("sequential").trim().to_ascii_lowercase().as_str() {
        "agentic" => ExecutionStrategy::Agentic,
        _ => ExecutionStrategy::Sequential,
    }
}

pub async fn planner_adjust_wizard_plan_op(
    state: &AppState,
    request: PlannerAdjustRequest,
) -> CommandResult<WizardAttackPlanDto> {
    let target = state
        .repositories()
        .targets()
        .get(&request.target_id)
        .await
        .map_err(CommandError::from)?;
    let profile = crate::commands::target_profile::parse_target_profile(&target.profile_json)?;
    if !profile.is_verified() {
        return Err(CommandError::invalid_input(
            "Target profile must be verified before adjusting attack plan",
        ));
    }

    let base = merge_adjust_base(&profile, &request);
    let categories = if request.profile_id.eq_ignore_ascii_case("custom") {
        Some(parse_attack_categories(&request.categories))
    } else {
        None
    };
    let mut plan = adjust_wizard_attack_plan(
        base,
        &request.profile_id,
        categories,
        &request.disabled_tests,
        &request.disabled_graph_nodes,
        request
            .payload_strategy
            .map(parse_payload_strategy)
            .transpose()?,
    );
    plan.execution_strategy = parse_execution_strategy(request.execution_strategy.as_deref());
    if let Some(max) = request.max_attempts {
        plan.max_attempts = max.clamp(1, 20);
    }
    if let Some(enabled) = request.reflection_enabled {
        plan.reflection_enabled = enabled;
    }
    if let Some(enabled) = request.adaptive_planning {
        plan.adaptive_planning = enabled;
    }
    plan.summary = build_wizard_plan_summary(&plan, &profile.full_url());
    Ok(wizard_plan_to_dto(plan))
}

#[tauri::command]
pub async fn attack_planner_adjust(
    state: State<'_, AppState>,
    request: PlannerAdjustRequest,
) -> CommandResult<WizardAttackPlanDto> {
    planner_adjust_wizard_plan_op(state.inner(), request).await
}
