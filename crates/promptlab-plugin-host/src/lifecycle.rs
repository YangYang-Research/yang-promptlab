use tracing::debug;

use crate::error::{PluginError, PluginResult};
use crate::types::PluginState;

/// Tracks plugin lifecycle transitions.
#[derive(Debug, Clone)]
pub struct PluginLifecycle {
    plugin_id: String,
    state: PluginState,
    history: Vec<(PluginState, Option<String>)>,
}

impl PluginLifecycle {
    pub fn new(plugin_id: impl Into<String>) -> Self {
        let id = plugin_id.into();
        Self {
            plugin_id: id.clone(),
            state: PluginState::Discovered,
            history: vec![(PluginState::Discovered, Some("discovered".into()))],
        }
    }

    pub fn state(&self) -> PluginState {
        self.state
    }

    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    pub fn transition(&mut self, next: PluginState, note: Option<String>) -> PluginResult<()> {
        if !self.state.can_transition_to(next) {
            return Err(PluginError::Lifecycle(format!(
                "plugin {} cannot transition {:?} -> {:?}",
                self.plugin_id, self.state, next
            )));
        }
        debug!(plugin_id = %self.plugin_id, from = ?self.state, to = ?next, "lifecycle transition");
        self.state = next;
        self.history.push((next, note));
        Ok(())
    }

    pub fn fail(&mut self, message: impl Into<String>) -> PluginResult<()> {
        let msg = message.into();
        self.state = PluginState::Error;
        self.history.push((PluginState::Error, Some(msg)));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_lifecycle_path() {
        let mut lc = PluginLifecycle::new("com.test.p");
        lc.transition(PluginState::Installed, None).unwrap();
        lc.transition(PluginState::Enabled, None).unwrap();
        lc.transition(PluginState::Loaded, None).unwrap();
        lc.transition(PluginState::Active, None).unwrap();
        lc.transition(PluginState::Loaded, None).unwrap();
        lc.transition(PluginState::Disabled, None).unwrap();
    }
}
