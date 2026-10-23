use crate::error::RegistryError;

/// One tool id the agents may load. Implementations stay next to the agent
/// that binds runtime state; this record is the catalog entry.
#[derive(Debug, Clone, Copy)]
pub struct ToolRecord {
    pub id: &'static str,
    pub description: &'static str,
}

const TOOLS: &[ToolRecord] = &[
    ToolRecord {
        id: "list_workspace",
        description: "List workspace projects.",
    },
    ToolRecord {
        id: "project_detail",
        description: "Read one project.",
    },
    ToolRecord {
        id: "list_scan",
        description: "List scans.",
    },
    ToolRecord {
        id: "scan_detail",
        description: "Read one scan.",
    },
    ToolRecord {
        id: "list_findings",
        description: "List findings.",
    },
    ToolRecord {
        id: "finding_detail",
        description: "Read one finding.",
    },
    ToolRecord {
        id: "list_targets",
        description: "List targets.",
    },
    ToolRecord {
        id: "target_detail",
        description: "Read one target.",
    },
    ToolRecord {
        id: "list_reports",
        description: "List reports.",
    },
    ToolRecord {
        id: "report_detail",
        description: "Read one report.",
    },
    ToolRecord {
        id: "create_project",
        description: "Create a project. Requires human approval.",
    },
    ToolRecord {
        id: "analyze_endpoint",
        description: "Decide whether a probed target is a live AI API.",
    },
    ToolRecord {
        id: "attack_plan",
        description: "Produce the attack plan for a verified target.",
    },
    ToolRecord {
        id: "generate_prompt",
        description: "Write a probe for one attack technique.",
    },
    ToolRecord {
        id: "recommend",
        description: "Write prioritized fixes for findings.",
    },
    ToolRecord {
        id: "summary",
        description: "Write the security posture of a project or scan.",
    },
    ToolRecord {
        id: "judge",
        description: "Decide whether a target reply is a finding.",
    },
    ToolRecord {
        id: "generate",
        description: "Generate payloads for the current attack attempt.",
    },
    ToolRecord {
        id: "attack",
        description: "Run the attack and judge for the current attempt.",
    },
    ToolRecord {
        id: "recover",
        description: "Adjust endpoint pacing after an unhealthy attack observation.",
    },
    ToolRecord {
        id: "reflect",
        description: "Ask ReflectionAgent whether to retry.",
    },
    ToolRecord {
        id: "adapt",
        description: "Adapt the attack plan before the next attempt.",
    },
    ToolRecord {
        id: "run_judge_worker",
        description: "Run JudgeWorker for this probe.",
    },
    ToolRecord {
        id: "run_classifier_worker",
        description: "Run ClassifierWorker for this probe.",
    },
    ToolRecord {
        id: "run_attacker_worker",
        description: "Run AttackerWorker for this probe.",
    },
    ToolRecord {
        id: "sequential_attack_execution_agent",
        description: "Delegate the scan to SequentialAttackExecutionAgent.",
    },
    ToolRecord {
        id: "agentic_attack_execution_agent",
        description: "Delegate the scan to AgenticAttackExecutionAgent.",
    },
];

pub fn all() -> &'static [ToolRecord] {
    TOOLS
}

pub fn load(id: &str) -> Result<&'static ToolRecord, RegistryError> {
    TOOLS
        .iter()
        .find(|tool| tool.id == id)
        .ok_or_else(|| RegistryError::UnknownTool(id.to_string()))
}
