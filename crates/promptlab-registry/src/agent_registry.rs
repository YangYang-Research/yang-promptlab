use crate::error::RegistryError;
use crate::prompt_registry::{self, LoadedPrompt};
use crate::tool_registry::{self, ToolRecord};

/// Declared agent. Prompt and tool ids point at the other two registries.
#[derive(Debug, Clone, Copy)]
pub struct AgentRecord {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub prompt_ids: &'static [&'static str],
    pub tool_ids: &'static [&'static str],
}

const AGENTS: &[AgentRecord] = &[
    AgentRecord {
        id: "yazg",
        display_name: "Yazg",
        description: "Answers workspace questions and runs studio actions.",
        prompt_ids: &[
            "yazg_system_prompt",
            "yazg_preamble_system_prompt",
            "yazg_conversation_system_prompt",
            "yazg_knowledge_system_prompt",
        ],
        tool_ids: &[
            "list_workspace",
            "project_detail",
            "list_scan",
            "scan_detail",
            "list_findings",
            "finding_detail",
            "list_targets",
            "target_detail",
            "list_reports",
            "report_detail",
            "create_project",
            "analyze_endpoint",
            "attack_plan",
            "generate_prompt",
            "recommend",
            "summary",
            "judge",
        ],
    },
    AgentRecord {
        id: "analyze_endpoint_agent",
        display_name: "AnalyzeEndpointAgent",
        description: "Decides whether a probed target is a live AI API.",
        prompt_ids: &["endpoint_verify_system_prompt"],
        tool_ids: &[],
    },
    AgentRecord {
        id: "attack_plan_agent",
        display_name: "AttackPlanAgent",
        description: "Writes the attack plan for a verified target.",
        prompt_ids: &["wizard_profile_system_prompt"],
        tool_ids: &[],
    },
    AgentRecord {
        id: "generate_prompt_agent",
        display_name: "GeneratePromptAgent",
        description: "Writes a probe for one attack technique.",
        prompt_ids: &["generator_system_prompt", "attack_catalog_prompt_system_prompt"],
        tool_ids: &[],
    },
    AgentRecord {
        id: "recommend_agent",
        display_name: "RecommendAgent",
        description: "Writes prioritized fixes for findings.",
        prompt_ids: &[
            "attack_results_recommend_system_prompt",
            "finding_remediation_recommend_system_prompt",
        ],
        tool_ids: &[],
    },
    AgentRecord {
        id: "summary_agent",
        display_name: "SummaryAgent",
        description: "Writes the security posture of a project or scan.",
        prompt_ids: &["project_summary_system_prompt", "scan_summary_system_prompt"],
        tool_ids: &[],
    },
    AgentRecord {
        id: "judge_coordinator_agent",
        display_name: "JudgeCoordinatorAgent",
        description: "Decides whether a target reply is a finding.",
        prompt_ids: &["judge_coordinator_system_prompt"],
        tool_ids: &[
            "run_judge_worker",
            "run_classifier_worker",
            "run_attacker_worker",
        ],
    },
    AgentRecord {
        id: "judge_worker",
        display_name: "JudgeWorker",
        description: "Votes on whether a target reply shows a successful attack.",
        prompt_ids: &["judge_system_prompt"],
        tool_ids: &[],
    },
    AgentRecord {
        id: "classifier_worker",
        display_name: "ClassifierWorker",
        description: "Assigns category and severity to a finding.",
        prompt_ids: &["classifier_system_prompt"],
        tool_ids: &[],
    },
    AgentRecord {
        id: "attacker_worker",
        display_name: "AttackerWorker",
        description: "Judges whether the target complied with an attack payload.",
        prompt_ids: &["attacker_system_prompt"],
        tool_ids: &[],
    },
    AgentRecord {
        id: "agentic_attack_execution_agent",
        display_name: "AgenticAttackExecutionAgent",
        description: "Pursues an attack plan across multiple attempts.",
        prompt_ids: &["agentic_attack_execution_system_prompt"],
        tool_ids: &["generate", "attack", "recover", "reflect", "adapt"],
    },
    AgentRecord {
        id: "sequential_attack_execution_agent",
        display_name: "SequentialAttackExecutionAgent",
        description: "Pursues an attack plan one category at a time.",
        prompt_ids: &["sequential_attack_execution_system_prompt"],
        tool_ids: &["generate", "attack", "recover"],
    },
    AgentRecord {
        id: "reflection_agent",
        display_name: "ReflectionAgent",
        description: "Reviews another agent answer and accepts or rejects it.",
        prompt_ids: &[
            "reflection_system_prompt",
            "agent_output_reflection_system_prompt",
        ],
        tool_ids: &[],
    },
    AgentRecord {
        id: "jailbreak_agent",
        display_name: "JailbreakAgent",
        description: "Writes prompts for one goal on a guarded model.",
        prompt_ids: &["jailbreak_agent_system_prompt", "jailbreak_followup_system_prompt"],
        tool_ids: &[],
    },
];

/// Agent loaded from [`load`]. Prompt and tool lookups stay inside the catalog.
#[derive(Debug, Clone, Copy)]
pub struct LoadedAgent {
    record: &'static AgentRecord,
}

impl LoadedAgent {
    pub fn id(self) -> &'static str {
        self.record.id
    }

    pub fn display_name(self) -> &'static str {
        self.record.display_name
    }

    pub fn description(self) -> &'static str {
        self.record.description
    }

    pub fn prompt_ids(self) -> &'static [&'static str] {
        self.record.prompt_ids
    }

    pub fn tool_ids(self) -> &'static [&'static str] {
        self.record.tool_ids
    }

    pub fn owns_prompt(self, prompt_id: &str) -> bool {
        self.record.prompt_ids.iter().any(|id| *id == prompt_id)
    }

    pub fn owns_tool(self, tool_id: &str) -> bool {
        self.record.tool_ids.iter().any(|id| *id == tool_id)
    }

    /// Primary system prompt. Agents with several prompts use [`Self::prompt`].
    pub fn system_prompt(self) -> Result<LoadedPrompt, RegistryError> {
        let id = self
            .record
            .prompt_ids
            .first()
            .copied()
            .ok_or(RegistryError::NoPrompt(self.record.id))?;
        self.prompt(id)
    }

    pub fn prompt(self, prompt_id: &str) -> Result<LoadedPrompt, RegistryError> {
        if !self.owns_prompt(prompt_id) {
            return Err(RegistryError::PromptNotOwned {
                agent: self.record.id,
                prompt: prompt_id.to_string(),
            });
        }
        prompt_registry::load(prompt_id)
    }

    pub fn tool(self, tool_id: &str) -> Result<&'static ToolRecord, RegistryError> {
        if !self.owns_tool(tool_id) {
            return Err(RegistryError::ToolNotOwned {
                agent: self.record.id,
                tool: tool_id.to_string(),
            });
        }
        tool_registry::load(tool_id)
    }

    pub fn tools(self) -> Result<Vec<&'static ToolRecord>, RegistryError> {
        self.record.tool_ids.iter().map(|id| self.tool(id)).collect()
    }
}

pub fn all() -> &'static [AgentRecord] {
    AGENTS
}

/// App entry. Load one agent by its registry id.
pub fn load(id: &str) -> Result<LoadedAgent, RegistryError> {
    AGENTS
        .iter()
        .find(|agent| agent.id == id)
        .map(|record| LoadedAgent { record })
        .ok_or_else(|| RegistryError::UnknownAgent(id.to_string()))
}
