use crate::types::{ConsensusReport, EvaluatorResult};

/// Multi-evaluator consensus aggregation.
pub struct ConsensusEngine;

impl ConsensusEngine {
    pub fn build_report(results: &[EvaluatorResult], vulnerable: bool) -> ConsensusReport {
        let participating = results.len();
        let vulnerable_votes = results.iter().filter(|r| r.vulnerable).count();
        let agreement_ratio = if participating == 0 {
            0.0
        } else {
            let agree = if vulnerable {
                vulnerable_votes
            } else {
                participating - vulnerable_votes
            };
            agree as f32 / participating as f32
        };

        ConsensusReport {
            agreement_ratio,
            participating_evaluators: participating,
            vulnerable_votes,
            dissent: agreement_ratio < 1.0 && participating > 1,
            method: "weighted_vote".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{EvaluatorKind, Severity};

    fn sample(vulnerable: bool) -> EvaluatorResult {
        EvaluatorResult {
            evaluator_id: "e".into(),
            kind: EvaluatorKind::Rule,
            role: None,
            vulnerable,
            confidence: 0.8,
            severity: Some(Severity::High),
            category: None,
            rationale: String::new(),
            indicators: vec![],
            structured: None,
        }
    }

    #[test]
    fn detects_dissent() {
        let results = vec![sample(true), sample(false)];
        let report = ConsensusEngine::build_report(&results, true);
        assert!(report.dissent);
        assert_eq!(report.vulnerable_votes, 1);
    }
}
