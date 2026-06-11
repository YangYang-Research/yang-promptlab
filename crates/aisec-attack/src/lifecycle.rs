use serde::{Deserialize, Serialize};

/// Lifecycle phase for a single attack run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttackPhase {
    Pending,
    Planning,
    Preparing,
    Executing,
    Evaluating,
    Collecting,
    Completed,
    Failed,
    Cancelled,
}

impl AttackPhase {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn can_transition_to(self, next: AttackPhase) -> bool {
        use AttackPhase::*;
        matches!(
            (self, next),
            (Pending, Planning)
                | (Planning, Preparing)
                | (Preparing, Executing)
                | (Executing, Evaluating)
                |             (Evaluating, Collecting)
                | (Evaluating, Executing) // multi-payload loop
                | (Collecting, Completed)
                | (Planning, Failed)
                | (Preparing, Failed)
                | (Executing, Failed)
                | (Evaluating, Failed)
                | (Collecting, Failed)
                | (_, Cancelled)
        )
    }
}

/// Event emitted during lifecycle transitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleEvent {
    pub probe_id: String,
    pub attack_id: String,
    pub phase: AttackPhase,
    pub message: Option<String>,
}

/// Tracks phase transitions for one attack execution.
#[derive(Debug, Clone)]
pub struct AttackLifecycle {
    phase: AttackPhase,
    probe_id: String,
    attack_id: String,
    events: Vec<LifecycleEvent>,
}

impl AttackLifecycle {
    pub fn new(probe_id: impl Into<String>, attack_id: impl Into<String>) -> Self {
        let probe_id = probe_id.into();
        let attack_id = attack_id.into();
        Self {
            phase: AttackPhase::Pending,
            probe_id: probe_id.clone(),
            attack_id: attack_id.clone(),
            events: vec![LifecycleEvent {
                probe_id,
                attack_id,
                phase: AttackPhase::Pending,
                message: Some("attack queued".into()),
            }],
        }
    }

    pub fn phase(&self) -> AttackPhase {
        self.phase
    }

    pub fn events(&self) -> &[LifecycleEvent] {
        &self.events
    }

    pub fn transition(
        &mut self,
        next: AttackPhase,
        message: Option<String>,
    ) -> crate::error::AttackResult<()> {
        if !self.phase.can_transition_to(next) {
            return Err(crate::error::AttackError::invalid_state(format!(
                "cannot transition from {:?} to {:?}",
                self.phase, next
            )));
        }
        self.phase = next;
        self.events.push(LifecycleEvent {
            probe_id: self.probe_id.clone(),
            attack_id: self.attack_id.clone(),
            phase: next,
            message,
        });
        Ok(())
    }

    pub fn fail(&mut self, message: impl Into<String>) -> crate::error::AttackResult<()> {
        let msg = message.into();
        if self.phase.is_terminal() {
            return Err(crate::error::AttackError::invalid_state(
                "attack already terminal",
            ));
        }
        self.phase = AttackPhase::Failed;
        self.events.push(LifecycleEvent {
            probe_id: self.probe_id.clone(),
            attack_id: self.attack_id.clone(),
            phase: AttackPhase::Failed,
            message: Some(msg),
        });
        Ok(())
    }

    pub fn complete(&mut self) -> crate::error::AttackResult<()> {
        self.transition(AttackPhase::Collecting, Some("collecting results".into()))?;
        self.transition(AttackPhase::Completed, Some("attack complete".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_lifecycle_transitions() {
        let mut lc = AttackLifecycle::new("p1", "prompt_injection");
        lc.transition(AttackPhase::Planning, None).unwrap();
        lc.transition(AttackPhase::Preparing, None).unwrap();
        lc.transition(AttackPhase::Executing, None).unwrap();
        lc.transition(AttackPhase::Evaluating, None).unwrap();
        lc.complete().unwrap();
        assert_eq!(lc.phase(), AttackPhase::Completed);
    }

    #[test]
    fn invalid_transition_rejected() {
        let mut lc = AttackLifecycle::new("p1", "jailbreak");
        assert!(lc.transition(AttackPhase::Executing, None).is_err());
    }
}
