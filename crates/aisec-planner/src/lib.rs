//! Shared attack-plan types and LLM bridge trait for AISec.

pub mod error;
pub mod local_llm;
pub mod normalize;
pub mod types;

pub use error::{PlannerError, PlannerResult};
pub use local_llm::PlannerLlm;
pub use normalize::parse_attack_category;
pub use types::{AttackPlan, CategoryRationale, PlannerMode};
