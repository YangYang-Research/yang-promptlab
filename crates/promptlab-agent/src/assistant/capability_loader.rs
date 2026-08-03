//! Loads the tool set for a resolved capability (never the global registry).

use super::capability_registry::{
    AssistantCapability, CapabilityRegistry, default_capability_registry,
};
use super::router::IntentResolution;

/// Tools selected for one assistant turn.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedCapabilityTools {
    pub capability: AssistantCapability,
    pub tool_names: Vec<&'static str>,
    pub allow_tool_calling: bool,
    pub system_addon: String,
    pub reason: String,
    pub confidence: f32,
}

impl LoadedCapabilityTools {
    pub fn tool_count(&self) -> usize {
        self.tool_names.len()
    }
}

/// Resolves capability → concrete tool name list for prompt serialization.
#[derive(Clone)]
pub struct CapabilityToolLoader {
    registry: CapabilityRegistry,
}

impl Default for CapabilityToolLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl CapabilityToolLoader {
    pub fn new() -> Self {
        Self {
            registry: default_capability_registry(),
        }
    }

    pub fn with_registry(registry: CapabilityRegistry) -> Self {
        Self { registry }
    }

    pub fn registry(&self) -> &CapabilityRegistry {
        &self.registry
    }

    /// Load tools for an already-resolved capability.
    pub fn load(&self, resolution: &IntentResolution) -> LoadedCapabilityTools {
        let capability = resolution.capability;
        let names = if capability.forces_no_tools() {
            Vec::new()
        } else {
            self.registry
                .tool_names(capability)
                .iter()
                .copied()
                .collect()
        };
        let allow_tool_calling = !capability.forces_no_tools() && !names.is_empty();
        LoadedCapabilityTools {
            capability,
            tool_names: names,
            allow_tool_calling,
            system_addon: self.registry.system_addon(capability).to_string(),
            reason: resolution.reason.clone(),
            confidence: resolution.confidence,
        }
    }

    /// Convenience: load by capability without router metadata.
    pub fn load_capability(&self, capability: AssistantCapability) -> LoadedCapabilityTools {
        self.load(&IntentResolution {
            capability,
            confidence: 1.0,
            reason: "direct".into(),
            raw_classifier_output: None,
            classifier_request: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // IntentRouter is LLM-based; loader tests use load_capability() directly.

    #[test]
    fn hello_loads_zero_tools() {
        let loader = CapabilityToolLoader::new();
        let loaded = loader.load_capability(AssistantCapability::Conversation);
        assert_eq!(loaded.capability, AssistantCapability::Conversation);
        assert_eq!(loaded.tool_count(), 0);
        assert!(!loaded.allow_tool_calling);
    }

    #[test]
    fn explain_prompt_injection_loads_zero_tools() {
        let loader = CapabilityToolLoader::new();
        let loaded = loader.load_capability(AssistantCapability::Knowledge);
        assert_eq!(loaded.capability, AssistantCapability::Knowledge);
        assert_eq!(loaded.tool_count(), 0);
    }

    #[test]
    fn list_projects_loads_projects_tools_only() {
        let loader = CapabilityToolLoader::new();
        let loaded = loader.load_capability(AssistantCapability::Projects);
        assert_eq!(loaded.capability, AssistantCapability::Projects);
        assert!(loaded.allow_tool_calling);
        assert!(loaded.tool_names.contains(&"list_workspace"));
        assert!(loaded.tool_names.contains(&"project_detail"));
        assert!(!loaded.tool_names.contains(&"analyze_endpoint"));
        assert!(!loaded.tool_names.contains(&"attack_plan"));
    }

    #[test]
    fn start_scan_loads_scan_tools_only() {
        let loader = CapabilityToolLoader::new();
        let loaded = loader.load_capability(AssistantCapability::Scan);
        assert_eq!(loaded.capability, AssistantCapability::Scan);
        assert_eq!(loaded.tool_names, vec!["list_scan", "scan_detail"]);
    }
}
