use crate::error::RegistryError;

/// One tool id the agents may load. Implementations stay next to the agent
/// that binds runtime state; this record is the catalog entry the agent reads.
#[derive(Debug, Clone, Copy)]
pub struct ToolRecord {
    pub id: &'static str,
    pub description: &'static str,
}

const TOOLS: &[ToolRecord] = &[
    ToolRecord {
        id: "list_workspace",
        description: concat!(
            "Inventory of every workspace project with aggregate counts only (targets, scans, findings). No arguments and no filter. ",
            "Use only when no project is named and the user wants the whole inventory. ",
            "Do not use when a project is named — call project_detail with that name. A full list is the wrong answer for one named project. ",
            "Not for targets, findings, scans, or reports.",
            " Do not call when the latest user message needs no live workspace data (conversation, identity, general knowledge, or simple reasoning without database rows).",
            " Returns JSON {status:ok|error|pending_approval, tool, data?, error_class?, message?, candidates?, hints?, retryable?}."
        ),
    },
    ToolRecord {
        id: "project_detail",
        description: concat!(
            "One project's metadata, targets, and scans. No finding rows. ",
            "Use whenever a project is named or implied. Prefer this over list_workspace once a name appears. ",
            "Argument project is the stored project id or name only. Strip labels such as \"project\" or \"dự án\" and any request verbs. ",
            "From \"thông tin project AI\" pass \"AI\". The value must equal the stored name or id. ",
            "On a miss: status=error, error_class=not_found, and candidates[]. Do not invent a project.",
            " Do not call when the latest user message needs no live workspace data (conversation, identity, general knowledge, or simple reasoning without database rows).",
            " Returns JSON {status:ok|error|pending_approval, tool, data?, error_class?, message?, candidates?, hints?, retryable?}."
        ),
    },
    ToolRecord {
        id: "list_scan",
        description: concat!(
            "Scans for one project, looked up by exact project id or name. ",
            "Use when the user asks about scans or runs of that project. ",
            "Not a target list (list_targets) and not a full project overview (project_detail). ",
            "One scan is scan_detail. On a miss: status=error, error_class=not_found, and candidates[].",
            " Do not call when the latest user message needs no live workspace data (conversation, identity, general knowledge, or simple reasoning without database rows).",
            " Returns JSON {status:ok|error|pending_approval, tool, data?, error_class?, message?, candidates?, hints?, retryable?}."
        ),
    },
    ToolRecord {
        id: "scan_detail",
        description: concat!(
            "One scan by scan id, including a capped finding list for that scan. ",
            "Use when the user names a scan. A single finding is finding_detail. ",
            "On a miss: status=error and error_class=not_found.",
            " Do not call when the latest user message needs no live workspace data (conversation, identity, general knowledge, or simple reasoning without database rows).",
            " Returns JSON {status:ok|error|pending_approval, tool, data?, error_class?, message?, candidates?, hints?, retryable?}."
        ),
    },
    ToolRecord {
        id: "list_findings",
        description: concat!(
            "Findings for a project and/or a scan, newest first, paginated. ",
            "Use only when the user asks for findings or vulnerabilities. ",
            "Provide project (id or name) and/or scan_id. Optional limit (default 20, max 50) and offset. ",
            "Finding number N is finding_detail with that project and index=N. Not a project overview and not a target list. ",
            "On a miss: status=error, error_class=not_found, and candidates[].",
            " Do not call when the latest user message needs no live workspace data (conversation, identity, general knowledge, or simple reasoning without database rows).",
            " Returns JSON {status:ok|error|pending_approval, tool, data?, error_class?, message?, candidates?, hints?, retryable?}."
        ),
    },
    ToolRecord {
        id: "finding_detail",
        description: concat!(
            "One finding, by finding_id, or by project (id or name) plus a 1-based index (newest first). ",
            "Use when the user asks for a specific finding by id or by number. ",
            "On a miss: status=error, error_class=not_found, and candidates[].",
            " Do not call when the latest user message needs no live workspace data (conversation, identity, general knowledge, or simple reasoning without database rows).",
            " Returns JSON {status:ok|error|pending_approval, tool, data?, error_class?, message?, candidates?, hints?, retryable?}."
        ),
    },
    ToolRecord {
        id: "list_targets",
        description: concat!(
            "AI targets for one project, looked up by exact project id or name. ",
            "Use when the user asks for targets or endpoints in a project. Prefer this over list_workspace. ",
            "Answer from the JSON. One target's profile is target_detail. ",
            "On a miss: status=error, error_class=not_found, and candidates[].",
            " Do not call when the latest user message needs no live workspace data (conversation, identity, general knowledge, or simple reasoning without database rows).",
            " Returns JSON {status:ok|error|pending_approval, tool, data?, error_class?, message?, candidates?, hints?, retryable?}."
        ),
    },
    ToolRecord {
        id: "target_detail",
        description: concat!(
            "One target by target_id, or by project plus name. ",
            "target_id also accepts a target name or host (for example an IP): resolved by id, then name, then host. ",
            "Use when the user asks for one specific target. ",
            "Returns only name, endpoint, host, and verified. A bare host or IP is only part of a target — report the endpoint. ",
            "Do not invent headers, templates, or capabilities. ",
            "On a miss: status=error, error_class=not_found, and candidates[].",
            " Do not call when the latest user message needs no live workspace data (conversation, identity, general knowledge, or simple reasoning without database rows).",
            " Returns JSON {status:ok|error|pending_approval, tool, data?, error_class?, message?, candidates?, hints?, retryable?}."
        ),
    },
    ToolRecord {
        id: "list_reports",
        description: concat!(
            "Generated reports for a project (id or name), or every project when project is omitted. ",
            "Use when the user asks about reports. One report is report_detail. ",
            "On a miss: status=error, error_class=not_found, and candidates[].",
            " Do not call when the latest user message needs no live workspace data (conversation, identity, general knowledge, or simple reasoning without database rows).",
            " Returns JSON {status:ok|error|pending_approval, tool, data?, error_class?, message?, candidates?, hints?, retryable?}."
        ),
    },
    ToolRecord {
        id: "report_detail",
        description: concat!(
            "One report by id: metadata plus a clipped content preview. ",
            "Use when the user asks to read a specific report. ",
            "On a miss: status=error, error_class=not_found, and candidates[].",
            " Do not call when the latest user message needs no live workspace data (conversation, identity, general knowledge, or simple reasoning without database rows).",
            " Returns JSON {status:ok|error|pending_approval, tool, data?, error_class?, message?, candidates?, hints?, retryable?}."
        ),
    },
    ToolRecord {
        id: "create_project",
        description: concat!(
            "A proposal to create a workspace project. Requires name. Description is optional. ",
            "Nothing is written until the user approves in chat: the result is status=pending_approval. ",
            "Call this for every create request. Do not treat chat history as proof the project exists. ",
            "Checks the workspace first and returns an error if the name already exists. ",
            "After pending_approval, reply with one short sentence, in the user's language, that approval is still required.",
            " Returns JSON {status:ok|error|pending_approval, tool, data?, error_class?, message?, candidates?, hints?, retryable?}."
        ),
    },
    ToolRecord {
        id: "analyze_endpoint",
        description: "AnalyzeEndpointAgent's decision on whether a bound live scan target is a generative AI API. Requires a bound target or capability_probe_ready=true (Scan wizard verification). Not for greetings, identity, general chat, or workspace inventory.",
    },
    ToolRecord {
        id: "attack_plan",
        description: "AttackPlanAgent's attack plan for a verified bound target: categories, techniques, execution mode, and payload policy. Requires verified=true or a bound verified target. Not for an unverified endpoint and not for writing a single probe.",
    },
    ToolRecord {
        id: "generate_prompt",
        description: "GeneratePromptAgent's probe for one attack technique in Attack Factory. Use only when factory_prompt_ready=true. A scan target is not required. Not a jailbreak follow-up and not a scan payload.",
    },
    ToolRecord {
        id: "recommend",
        description: "RecommendAgent's prioritized fixes for completed attack results. Requires attack_results_ready=true. Not a scan summary and not a verdict on one reply.",
    },
    ToolRecord {
        id: "summary",
        description: "SummaryAgent's security posture for a project or a scan, plus highlights. Requires summary_ready=true. Not a remediation list and not a finding verdict.",
    },
    ToolRecord {
        id: "judge",
        description: "JudgeCoordinatorAgent's decision on whether a target reply is a finding. It collects one vote each from JudgeWorker, ClassifierWorker, and AttackerWorker. Requires judge_ready=true and the probe plus response. Not a workspace lookup.",
    },
    ToolRecord {
        id: "generate",
        description: "Payloads for this attack attempt. Call this before attack. Call it again when the previous payloads are spent and another attempt starts. Does not send traffic and does not judge the reply.",
    },
    ToolRecord {
        id: "attack",
        description: "This attempt's payloads sent to the target, plus a judge of the reply. Requires generate first. Does not rewrite payloads and does not change endpoint pacing.",
    },
    ToolRecord {
        id: "recover",
        description: "Endpoint pacing after an unhealthy attack observation (failure or timeout). Not for a refusal or a judged reply. Does not generate payloads.",
    },
    ToolRecord {
        id: "reflect",
        description: "ReflectionAgent's accept-or-retry decision after a healthy attack observation. Use only when reflection is enabled. Does not send the attack and does not change the plan.",
    },
    ToolRecord {
        id: "adapt",
        description: "A revised attack plan before the next attempt. Use only after reflect says retry and adaptive planning is enabled. Does not send traffic.",
    },
    ToolRecord {
        id: "run_judge_worker",
        description: "JudgeWorker's vote on whether this target reply shows a successful attack. Call once for this probe. Returns a vulnerable flag and a confidence. Does not assign category or severity.",
    },
    ToolRecord {
        id: "run_classifier_worker",
        description: "ClassifierWorker's vote: vulnerability category and severity for this probe. Call once. Does not make the final success verdict.",
    },
    ToolRecord {
        id: "run_attacker_worker",
        description: "AttackerWorker's vote on whether the target complied with the attack payload. Call once for this probe. Does not assign category or severity.",
    },
    ToolRecord {
        id: "sequential_attack_execution_agent",
        description: "SequentialAttackExecutionAgent for a scan whose execution mode is sequential. It pursues the plan one category at a time: generate, then attack and judge, and recover only when the endpoint fails. No reflection loop and no plan rewrite.",
    },
    ToolRecord {
        id: "agentic_attack_execution_agent",
        description: "AgenticAttackExecutionAgent for a scan whose execution mode is agentic. It pursues the plan across attempts: generate, attack and judge, reflect, then adapt before the next attempt.",
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

/// Description the owning agent shows the model. Unknown ids panic: the catalog is the source.
pub fn description(id: &str) -> &'static str {
    load(id)
        .unwrap_or_else(|_| panic!("unknown tool: {id}"))
        .description
}
