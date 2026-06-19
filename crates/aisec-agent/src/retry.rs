use aisec_generator::GeneratorMode;

use crate::types::AgentConfig;

/// Escalate payload generation strategy on each retry.
pub fn generator_mode_for_retry(config: &AgentConfig, retry_index: u32) -> GeneratorMode {
    match retry_index {
        0 => config.initial_generator_mode,
        1 => GeneratorMode::TemplateMutation,
        _ => GeneratorMode::LocalLlm,
    }
}

/// Whether the agent should retry after a non-vulnerable judge outcome.
pub fn should_retry(
    vulnerable: bool,
    attempt: u32,
    config: &AgentConfig,
) -> bool {
    !vulnerable && attempt < config.max_attempts_per_category as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use aisec_generator::GeneratorMode;
    use aisec_planner::PlannerMode;

    #[test]
    fn escalates_generator_modes() {
        let config = AgentConfig {
            max_attempts_per_category: 5,
            planner_mode: PlannerMode::Deterministic,
            initial_generator_mode: GeneratorMode::StaticPack,
        };
        assert_eq!(
            generator_mode_for_retry(&config, 0),
            GeneratorMode::StaticPack
        );
        assert_eq!(
            generator_mode_for_retry(&config, 1),
            GeneratorMode::TemplateMutation
        );
        assert_eq!(
            generator_mode_for_retry(&config, 2),
            GeneratorMode::LocalLlm
        );
    }

    #[test]
    fn stops_when_vulnerable_or_budget_exhausted() {
        let config = AgentConfig::default();
        assert!(!should_retry(true, 1, &config));
        assert!(should_retry(false, 1, &config));
        assert!(!should_retry(false, 5, &config));
    }
}
