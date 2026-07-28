//! Yazg ReAct tool specs — name + description + JSON Schema parameters.
//!
//! Bound on the supervisor LLM via LangChain-style tool calling so the model
//! chooses tools by reading descriptions (not keyword heuristics).
//! <https://www.langchain.com/blog/tool-calling-with-langchain>

use promptlab_planner::ToolSpec;
use serde_json::json;

/// Tools available to the Yazg supervisor on each ReAct step.
pub fn yazg_react_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec::new(
            "list_workspace",
            "Read projects, targets, scans, and findings from the local PromptLab database. \
             Use for workspace inventory, listing what exists, and finding/vulnerability counts \
             for a named project (including questions like \"how many findings\" or Vietnamese \
             \"số lỗ hổng\"). Does NOT require a live scan target. Never invent rows.",
            json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        ),
        ToolSpec::new(
            "analyze_endpoint",
            "Probe/classify whether a bound live scan target is a generative AI API \
             (AnalyzeEndpointAgent). Requires a bound target or capability_probe_ready=true \
             (Scan wizard Verification). Do NOT use for counting existing findings, \
             project inventory, or general chat.",
            json!({
                "type": "object",
                "properties": {
                    "thought": { "type": "string", "description": "Brief reasoning" }
                },
                "additionalProperties": false
            }),
        ),
        ToolSpec::new(
            "attack_plan",
            "Build an attack plan for a verified bound target (AttackPlanAgent). \
             Requires verified=true (or a bound verified target). Do not call before \
             the endpoint is verified unless context already says verified=true.",
            json!({
                "type": "object",
                "properties": {
                    "thought": { "type": "string", "description": "Brief reasoning" }
                },
                "additionalProperties": false
            }),
        ),
        ToolSpec::new(
            "generate_prompt",
            "Attack Factory: invent a novel technique probe (GeneratePromptAgent). \
             Use only when factory_prompt_ready=true and the user wants Attack Factory work. \
             Does not require a scan target.",
            json!({
                "type": "object",
                "properties": {
                    "thought": { "type": "string", "description": "Brief reasoning" }
                },
                "additionalProperties": false
            }),
        ),
        ToolSpec::new(
            "recommend",
            "Post-scan remediation recommendations from completed attack results \
             (RecommendAgent). Requires attack_results_ready=true. No live target probe needed.",
            json!({
                "type": "object",
                "properties": {
                    "thought": { "type": "string", "description": "Brief reasoning" }
                },
                "additionalProperties": false
            }),
        ),
        ToolSpec::new(
            "summary",
            "Project or scan posture overview + highlights (SummaryAgent). \
             Requires summary_ready=true. No live target probe needed.",
            json!({
                "type": "object",
                "properties": {
                    "thought": { "type": "string", "description": "Brief reasoning" }
                },
                "additionalProperties": false
            }),
        ),
        ToolSpec::new(
            "judge",
            "Consensus judging via JudgeCoordinatorAgent (JudgeWorker + ClassifierWorker + \
             AttackerWorker). Requires judge_ready=true and probe/response context.",
            json!({
                "type": "object",
                "properties": {
                    "thought": { "type": "string", "description": "Brief reasoning" }
                },
                "additionalProperties": false
            }),
        ),
        ToolSpec::new(
            "create_project",
            "Create a workspace project in the local database. Requires a project name. \
             Optional description. Do NOT ask for a scan target and do NOT call \
             analyze_endpoint for project creation.",
            json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Project name to create"
                    },
                    "description": {
                        "type": "string",
                        "description": "Optional project description"
                    },
                    "thought": { "type": "string", "description": "Brief reasoning" }
                },
                "required": ["name"],
                "additionalProperties": false
            }),
        ),
        ToolSpec::new(
            "finish",
            "Stop the ReAct loop and answer the user with a final reply. \
             Use when you have enough information (including after tool Observations) \
             or for general conversational answers that need no specialist tool.",
            json!({
                "type": "object",
                "properties": {
                    "reply": {
                        "type": "string",
                        "description": "Final user-visible answer"
                    },
                    "thought": { "type": "string", "description": "Brief reasoning" }
                },
                "required": ["reply"],
                "additionalProperties": false
            }),
        ),
    ]
}
