//! Dynamic attack plan generation from AISec fingerprint results.

pub mod deterministic;
pub mod engine;
pub mod error;
pub mod local_llm;
pub mod normalize;
pub mod types;

pub use deterministic::plan_deterministic;
pub use engine::generate_attack_plan;
pub use error::{PlannerError, PlannerResult};
pub use local_llm::{plan_with_local_llm, PlannerLlm};
pub use normalize::{normalize_fingerprint_category, parse_attack_category};
pub use types::{
    AttackPlan, CategoryRationale, FingerprintEndpoint, FingerprintResult, PlannerMode,
};
