# Yazg and AgentTrace

**Last verified:** 2026-08-23

Yazg (`promptlab-agent`) is the **manager**. Chat and product workflows go through `YazgSupervisor` (ReAct).

- **Sub-agent** — specialist with its own `AgentId`, prompt, and (usually) LLM loop.
- **Tool** — a ReAct callable on Yazg or a scan orchestrator. Workspace tools hit SQLite. Specialist tools **delegate** to a sub-agent. Scan **host** tools are harness/generator (no LLM).

Sub-agents do not call each other as peers.

```
Chat / wizard / scan host
  → YazgSupervisor (intent hint is soft)
      → tool call
          → workspace: SQLite / HILT
          → specialist tool → sub-agent
              → host tools (harness HTTP, generator, judge engine)
```

IDs: `AgentId` in `crates/promptlab-agent/src/types.rs`. Tools: `yazg_tools.rs`. Token counters: Settings → Usage (`known_agent_ids()`). UI copy: `src/features/settings/runtimeInferenceSites.ts`.

Hard-gate scan runs: `execute_attack` / `execute_sequential_attack` after `pick_scan_execution_agent`. Typed product entrypoints: `react_classify_probe`, `react_plan`, `react_generate_prompt`, `react_recommend`, `react_summarize`, `react_judge`.

Chat IPC: `yazg_chat`, `yazg_stop`, `yazg_generate_chat_title`, `yazg_resolve_hilt`. Scan/wizard: [ATTACK.md](ATTACK.md), [DISCOVERY.md](DISCOVERY.md).

---

## Sub-agents

| `AgentId` | Display | Role | Host |
|-----------|---------|------|------|
| `yazg` | Yazg | Supervisor ReAct | All turns |
| `analyze_endpoint` | AnalyzeEndpointAgent | Classify HTTP snapshot as live AI API | Wizard verify (`react_classify_probe`) |
| `attack_plan` | AttackPlanAgent | Categories, techniques, strategy, payload policy (+ adapt on retry) | Wizard plan (`react_plan`) |
| `generate_prompt` | GeneratePromptAgent | Novel catalog probe for a technique | Attack Factory (`react_generate_prompt`) |
| `recommend` | RecommendAgent | Post-scan remediations | `react_recommend` |
| `summary` | SummaryAgent | Project / scan posture blurb | `react_summarize` |
| `judge_coordinator` | JudgeCoordinatorAgent | ReAct → role workers → consensus | Scan judge + `react_judge` |
| `judge_worker` | JudgeWorker | Judge-role vote | Under coordinator |
| `classifier_worker` | ClassifierWorker | Classifier-role vote | Under coordinator |
| `attacker_worker` | AttackerWorker | Attacker-role vote | Under coordinator |
| `sequential_attack_execution` | SequentialAttackExecutionAgent | ReAct: generate → attack(+judge) → recover | `scan_start` sequential |
| `agentic_attack_execution` | AgenticAttackExecutionAgent | ReAct: generate → attack → reflect → adapt → retry | `scan_start` agentic |
| `reflection` | ReflectionAgent | Structured retry extractor (heuristic fallback) | Inside agentic loop |

`create_project` / `list_workspace` are **tools**, not agents (`AgentId` still used for STM/events).

### Scan pick

`pick_scan_execution_agent` (plan + runtime):

| Agent | Loop |
|-------|------|
| Sequential | One generate → attack(+JudgeCoordinator) per category. Recover on transport failure. **No** reflection/adapt |
| Agentic | Same plus ReflectionAgent → `AttackPlanAgent::adapt` → retry until hit or `max_agent_attempts` (default 5) |

Playbook: `agent_mode`, `max_agent_attempts`. Endpoint pacing/recovery is shared (`endpoint_recovery`).

---

## Tools

### Workspace (SQLite / HILT)

No sub-agent. Code: `yazg_tools.rs`. Mutating `create_project` is HILT (below).

| Tool | Capability | Role |
|------|------------|------|
| `list_workspace` | Workspace, Projects | Inventory + counts |
| `project_detail` | Projects | One project |
| `create_project` | Projects | Create (HILT) |
| `list_targets` | Targets | Target list |
| `target_detail` | Targets | One target |
| `list_scan` | Scan | Scan list |
| `scan_detail` | Scan | One scan |
| `list_findings` | Findings | Finding list |
| `finding_detail` | Findings | One finding |
| `list_reports` | Reports | Report list |
| `report_detail` | Reports | One report |

Models / Runtime / Settings: **no tools** (reserved empty). Conversation / Knowledge: zero tools.

### Specialist (tool → sub-agent)

Yazg ReAct calls the **tool**; the tool runs the **sub-agent**.

| Tool | Sub-agent | Capability |
|------|-----------|------------|
| `analyze_endpoint` | AnalyzeEndpointAgent | Targets |
| `attack_plan` | AttackPlanAgent | Attack |
| `generate_prompt` | GeneratePromptAgent | Attack |
| `recommend` | RecommendAgent | Attack |
| `summary` | SummaryAgent | Attack |
| `judge` | JudgeCoordinatorAgent | Attack |

### Scan orchestrator (pick + host)

Not registered on chat Yazg. Sequential/agentic agents ReAct-pick an action; the **host** (`AttackExecutionTools` in `src-tauri` scan execution) runs it.

| Pick tool | Host | Notes |
|-----------|------|-------|
| `generate` | `generate_payloads` | Catalog / mutator / local LLM |
| `attack` | `run_attack_attempt` | Harness HTTP + JudgeCoordinator (cancel/pause) |
| `recover` | `apply_pacing` / backoff | Transport failure |
| `reflect` | ReflectionAgent | Agentic only |
| `adapt` | `apply_adapt` → AttackPlanAgent | Agentic only |
| `sequential_attack_execution` / `agentic_attack_execution` | Supervisor pick | Yazg chooses scan agent |

HTTP delivery is **never** a Yazg chat tool. Judge crate: [ATTACK.md](ATTACK.md#judge-promptlab-judge).

---

## Chat capability routing

Assistant chat **classifies capability first** — tools are not injected wholesale.

```
User message
  → LLM #1 IntentRouter (text only) → { capability, confidence, reason }
  → CapabilityToolLoader
  → LLM #2 Yazg (history + that capability’s tools only)
  → optional tool call → response
```

Classifier input is **only the latest user message**. Greeting/knowledge → zero tools (cannot call `analyze_endpoint`). Classifier failure / bad JSON → **Conversation**.

Registry is **builtin only** (`CapabilityRegistry::builtin`). Code: `crates/promptlab-agent/src/assistant/` + `yazg_runtime.rs` (`run_yazg`) + `supervisor.rs`.

### HILT (human-in-the-loop)

Mutating tools (`create_project`, `create_*` / `update_*` / `delete_*`) do **not** write immediately. Host stores `HiltPendingAction` (TTL **15 min**). Chat shows Approve / Deny; `yazg_resolve_hilt` applies or discards, then a follow-up LLM turn. Expired/denied → no write.

### Memory

STM (`agent_short_term_memory`, session-scoped) and LTM (`agent_long_term_memory`, keyed upsert). IPC: `agent_memory_list_sessions/events/ltm`, `agent_memory_delete_session`. Insights may be promoted STM → LTM after a turn.

---

## AgentTrace (`promptlab-agenttrace`)

UI reads **AgentTrace SQLite**, not `agents.log` (dual-write remains for debug).

| Concept | Meaning |
|---------|---------|
| Experiment (`yazg`) | Bucket |
| Trace | One Yazg turn |
| Span | `capability_classify`, `llm`, `tool:*` |
| `session_id` | `yazg-chat:<threadId>` |

Path: `~/.promptlab/agenttrace/agenttrace.db` (`at_experiments`, `at_traces`, `at_spans`).

`run_yazg` records classifier I/O, per-completion wire bodies, tool spans. LLM spans store token metrics (`totalTokens` on list/detail). Deleting an Assistant conversation deletes AgentTrace rows + STM for that session. Trace UI only lists sessions that still exist in Assistant.

IPC: `agenttrace_list_sessions`, `agenttrace_list_traces`, `agenttrace_get_trace`, `agenttrace_delete_session`.
