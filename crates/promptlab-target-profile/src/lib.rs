//! AI Target Profile — single source of truth for scan wizard endpoint configuration.
//!
//! Defines how requests are built (profile), how they are sent (harness mapping),
//! and what capabilities the planner consumes. No discovery, fingerprinting, or
//! schema inference.

pub mod capabilities;
pub mod harness;
pub mod payload_strategy;
pub mod planner;
pub mod wizard_llm;
pub mod wizard_plan;
pub mod wizard_recommendations;
pub mod wizard_summary;
pub mod prompt;
pub mod serde_verified_at;
pub mod templates;
pub mod types;
pub mod verification;
pub mod verification_llm;

pub use capabilities::{default_capabilities_for_provider, effective_capabilities};
pub use harness::harness_kind_for_profile;
pub use payload_strategy::{
    all_attack_mutator_ids, capability_influences_strategy, payload_strategy_for_attack_profile,
    recommend_payload_strategy, MutationLevel, PayloadGenerationStrategy, PayloadStrategy,
};
pub use planner::{plan_from_target_profile, summary_for_api_endpoint};
pub use wizard_llm::build_wizard_attack_plan_with_llm;
pub use wizard_recommendations::{
    build_attack_results_summary, ensure_failed_scan_action_recommendation,
    generate_attack_recommendations_with_llm, generate_finding_recommendations_with_llm,
    is_retryable_scan_status, parse_attack_recommendations, AttackRecommendation,
    AttackRecommendationsBundle, AttackResultsSummary, FindingRemediationInput,
    FindingSummaryInput,
};
pub use wizard_summary::{
    ensure_failed_project_summary_action, generate_project_summary_with_llm,
    generate_scan_summary_with_llm, parse_summary_bundle, project_has_retryable_scan_status,
    SummaryAction, SummaryBundle,
};
pub use wizard_plan::{
    adjust_wizard_attack_plan, active_categories_for_profile, apply_profile_mode_settings,
    build_deterministic_profile_modes, build_wizard_attack_plan, build_wizard_plan_summary,
    find_profile_mode, union_mode_categories, AttackProfileMode, ExecutionStrategy,
    WizardAttackPlan,
};
pub use prompt::{contains_prompt_placeholder, replace_prompt, PROMPT_PLACEHOLDER};
pub use templates::{list_provider_templates, template_for_provider};
pub use types::*;
pub use verification::{
    execute_capability_probe, execute_verify_http, execute_verify_http_with_prompt, has_ai_response,
    verify_target_profile, CONNECT_PROBE_PROMPT, VERIFY_PROMPT, VerificationAttempt,
    VerificationConsoleEntry, VerificationError, VerifyHttpSuccess,
};
pub use verification_llm::{
    validate_http_response_with_llm, verify_target_profile_with_llm,
};
