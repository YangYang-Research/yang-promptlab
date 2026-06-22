//! Runtime lifecycle state machine — never use booleans for status.

use serde::{Deserialize, Serialize};

/// Full lifecycle states for the embedded inference runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeLifecycleState {
    NotInstalled,
    Downloading,
    Installing,
    Installed,
    Starting,
    Running,
    Busy,
    Stopping,
    Stopped,
    Updating,
    Failed,
}

impl RuntimeLifecycleState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotInstalled => "not_installed",
            Self::Downloading => "downloading",
            Self::Installing => "installing",
            Self::Installed => "installed",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Busy => "busy",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Updating => "updating",
            Self::Failed => "failed",
        }
    }

    pub fn is_operational(self) -> bool {
        matches!(
            self,
            Self::Installed | Self::Starting | Self::Running | Self::Busy | Self::Stopped
        )
    }

    pub fn can_start(self) -> bool {
        matches!(self, Self::Installed | Self::Stopped | Self::Failed)
    }

    pub fn can_stop(self) -> bool {
        matches!(self, Self::Running | Self::Busy | Self::Starting)
    }
}

/// Validated state transition.
pub fn transition(from: RuntimeLifecycleState, to: RuntimeLifecycleState) -> RuntimeLifecycleState {
    use RuntimeLifecycleState as S;
    match (from, to) {
        (S::NotInstalled, S::Downloading) => to,
        (S::Downloading, S::Installing) => to,
        (S::Installing, S::Installed) => to,
        (S::Installed, S::Starting) => to,
        (S::Starting, S::Running) => to,
        (S::Running, S::Busy) => to,
        (S::Busy, S::Running) => to,
        (S::Running, S::Stopping) => to,
        (S::Busy, S::Stopping) => to,
        (S::Starting, S::Stopping) => to,
        (S::Stopping, S::Stopped) => to,
        (S::Stopped, S::Starting) => to,
        (S::Installed, S::Updating) => to,
        (S::Updating, S::Downloading) => to,
        (_, S::Failed) => to,
        (S::Failed, S::NotInstalled) => to,
        (S::Failed, S::Downloading) => to,
        (S::Failed, S::Installed) => to,
        (S::Stopped, S::Installed) => to,
        (S::Running, S::Installed) => to,
        (current, _) => current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_flow_transitions() {
        let s = RuntimeLifecycleState::NotInstalled;
        let s = transition(s, RuntimeLifecycleState::Downloading);
        let s = transition(s, RuntimeLifecycleState::Installing);
        let s = transition(s, RuntimeLifecycleState::Installed);
        assert_eq!(s, RuntimeLifecycleState::Installed);
    }
}
