//! Assistant capability registry — maps capabilities to owned tool sets.
//!
//! Capabilities are dynamically registerable so plugins / MCP / enterprise packs
//! can extend the registry without touching AI Runtime.

use std::collections::HashMap;

/// High-level assistant capability (tool ownership boundary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssistantCapability {
    Conversation,
    Workspace,
    Models,
    Runtime,
    Projects,
    Targets,
    Scan,
    Findings,
    Reports,
    Attack,
    Knowledge,
    Settings,
}

impl AssistantCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Conversation => "conversation",
            Self::Workspace => "workspace",
            Self::Models => "models",
            Self::Runtime => "runtime",
            Self::Projects => "projects",
            Self::Targets => "targets",
            Self::Scan => "scan",
            Self::Findings => "findings",
            Self::Reports => "reports",
            Self::Attack => "attack",
            Self::Knowledge => "knowledge",
            Self::Settings => "settings",
        }
    }

    /// Tie-break priority when scores are equal (lower index = preferred).
    pub fn priority(self) -> u8 {
        match self {
            Self::Conversation => 0,
            Self::Knowledge => 1,
            Self::Projects => 2,
            Self::Targets => 3,
            Self::Scan => 4,
            Self::Findings => 5,
            Self::Runtime => 6,
            Self::Models => 7,
            Self::Reports => 8,
            Self::Settings => 9,
            Self::Attack => 10,
            Self::Workspace => 11,
        }
    }

    /// Capabilities that must never advertise tools / force no function calling.
    pub fn forces_no_tools(self) -> bool {
        matches!(self, Self::Conversation | Self::Knowledge)
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Conversation,
            Self::Knowledge,
            Self::Projects,
            Self::Targets,
            Self::Scan,
            Self::Findings,
            Self::Runtime,
            Self::Models,
            Self::Reports,
            Self::Settings,
            Self::Attack,
            Self::Workspace,
        ]
    }
}

impl std::fmt::Display for AssistantCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Static definition for one capability.
#[derive(Debug, Clone)]
pub struct CapabilityDefinition {
    pub capability: AssistantCapability,
    /// Existing Yazg Rig tool names owned by this capability.
    pub tool_names: &'static [&'static str],
    /// Extra system guidance appended for this capability (not STM/LTM).
    pub system_addon: &'static str,
}

/// Registry of capabilities → tool ownership.
///
/// New capabilities (plugins / MCP) register via [`CapabilityRegistry::register`].
#[derive(Clone, Default)]
pub struct CapabilityRegistry {
    entries: HashMap<AssistantCapability, CapabilityDefinition>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, def: CapabilityDefinition) {
        self.entries.insert(def.capability, def);
    }

    pub fn get(&self, capability: AssistantCapability) -> Option<&CapabilityDefinition> {
        self.entries.get(&capability)
    }

    pub fn tool_names(&self, capability: AssistantCapability) -> &[&'static str] {
        self.entries
            .get(&capability)
            .map(|d| d.tool_names)
            .unwrap_or(&[])
    }

    pub fn system_addon(&self, capability: AssistantCapability) -> &'static str {
        self.entries
            .get(&capability)
            .map(|d| d.system_addon)
            .unwrap_or("")
    }

    pub fn builtin() -> Self {
        let mut reg = Self::new();
        for def in BUILTIN_DEFINITIONS {
            reg.register(def.clone());
        }
        reg
    }
}

/// Process-wide default registry accessor (builtin Yazg tools).
pub fn default_capability_registry() -> CapabilityRegistry {
    CapabilityRegistry::builtin()
}

const BUILTIN_DEFINITIONS: &[CapabilityDefinition] = &[
    CapabilityDefinition {
        capability: AssistantCapability::Conversation,
        tool_names: &[],
        system_addon: "Capability: Conversation.\n\
Reply naturally. Greeting, small talk, identity, and general reasoning only.\n\
Do not call tools. Do not invent workspace data.",
    },
    CapabilityDefinition {
        capability: AssistantCapability::Knowledge,
        tool_names: &[],
        system_addon: "Capability: Knowledge.\n\
Answer from general AI/security knowledge (prompt injection, OWASP LLM, architecture).\n\
Do not call tools. Do not invent project/target/scan rows.",
    },
    CapabilityDefinition {
        capability: AssistantCapability::Workspace,
        tool_names: &["list_workspace"],
        system_addon: "Capability: Workspace.\n\
Use list_workspace for inventory of projects and aggregate counts only.",
    },
    CapabilityDefinition {
        capability: AssistantCapability::Projects,
        tool_names: &["list_workspace", "project_detail", "create_project"],
        system_addon: "Capability: Projects.\n\
Use list_workspace / project_detail / create_project for project questions.",
    },
    CapabilityDefinition {
        capability: AssistantCapability::Targets,
        tool_names: &["list_targets", "target_detail", "analyze_endpoint"],
        system_addon: "Capability: Targets.\n\
Use list_targets / target_detail for target inventory.\n\
Use analyze_endpoint only when a live bound target or capability probe is ready.",
    },
    CapabilityDefinition {
        capability: AssistantCapability::Scan,
        tool_names: &["list_scan", "scan_detail"],
        system_addon: "Capability: Scan.\n\
Use list_scan / scan_detail for scan runs.",
    },
    CapabilityDefinition {
        capability: AssistantCapability::Findings,
        tool_names: &["list_findings", "finding_detail"],
        system_addon: "Capability: Findings.\n\
Use list_findings / finding_detail when the user asks about findings / vulnerabilities.",
    },
    CapabilityDefinition {
        capability: AssistantCapability::Reports,
        tool_names: &["list_reports", "report_detail"],
        system_addon: "Capability: Reports.\n\
Use list_reports / report_detail for generated reports.",
    },
    CapabilityDefinition {
        capability: AssistantCapability::Attack,
        tool_names: &[
            "attack_plan",
            "generate_prompt",
            "recommend",
            "summary",
            "judge",
        ],
        system_addon: "Capability: Attack / specialists.\n\
Use attack_plan, generate_prompt, recommend, summary, or judge only when runtime flags say ready.",
    },
    CapabilityDefinition {
        capability: AssistantCapability::Models,
        tool_names: &[],
        system_addon: "Capability: Models.\n\
Model install/list tools are not wired in this build — explain what you can do and ask for Settings → Models.",
    },
    CapabilityDefinition {
        capability: AssistantCapability::Runtime,
        tool_names: &[],
        system_addon: "Capability: Runtime.\n\
Runtime start/stop tools are not wired in this build — point the user to AI Runtime settings.",
    },
    CapabilityDefinition {
        capability: AssistantCapability::Settings,
        tool_names: &[],
        system_addon: "Capability: Settings.\n\
Settings mutation tools are not wired in this build — guide the user to the Settings UI.",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_and_knowledge_own_zero_tools() {
        let reg = CapabilityRegistry::builtin();
        assert!(reg.tool_names(AssistantCapability::Conversation).is_empty());
        assert!(reg.tool_names(AssistantCapability::Knowledge).is_empty());
        assert!(AssistantCapability::Conversation.forces_no_tools());
        assert!(AssistantCapability::Knowledge.forces_no_tools());
    }

    #[test]
    fn projects_owns_project_tools_only() {
        let reg = CapabilityRegistry::builtin();
        let tools = reg.tool_names(AssistantCapability::Projects);
        assert!(tools.contains(&"list_workspace"));
        assert!(tools.contains(&"project_detail"));
        assert!(tools.contains(&"create_project"));
        assert!(!tools.contains(&"analyze_endpoint"));
        assert!(!tools.contains(&"attack_plan"));
    }

    #[test]
    fn scan_does_not_include_attack_tools() {
        let reg = CapabilityRegistry::builtin();
        let tools = reg.tool_names(AssistantCapability::Scan);
        assert_eq!(tools, &["list_scan", "scan_detail"]);
    }

    #[test]
    fn register_extends_registry() {
        let mut reg = CapabilityRegistry::builtin();
        reg.register(CapabilityDefinition {
            capability: AssistantCapability::Models,
            tool_names: &["list_models"],
            system_addon: "plugin models",
        });
        assert_eq!(
            reg.tool_names(AssistantCapability::Models),
            &["list_models"]
        );
    }
}
