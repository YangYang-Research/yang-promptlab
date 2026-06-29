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
pub mod prompt;
pub mod serde_verified_at;
pub mod templates;
pub mod types;
pub mod verification;

pub use capabilities::default_capabilities_for_provider;
pub use harness::harness_kind_for_profile;
pub use payload_strategy::{
    capability_influences_strategy, payload_strategy_for_attack_profile,
    recommend_payload_strategy, MutationLevel, PayloadGenerationStrategy, PayloadStrategy,
};
pub use planner::{plan_from_target_profile, summary_for_api_endpoint};
pub use wizard_llm::build_wizard_attack_plan_with_llm;
pub use wizard_plan::{
    adjust_wizard_attack_plan, active_categories_for_profile, build_wizard_attack_plan,
    build_wizard_plan_summary, ExecutionStrategy, WizardAttackPlan,
};
pub use prompt::{contains_prompt_placeholder, replace_prompt, PROMPT_PLACEHOLDER};
pub use templates::{list_provider_templates, template_for_provider};
pub use types::*;
pub use verification::{
    verify_target_profile, VerificationAttempt, VerificationConsoleEntry, VerificationError,
};
