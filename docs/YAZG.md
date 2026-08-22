# Yazg and AgentTrace

**Last verified:** 2026-08-22

Yazg is the in-app assistant (`promptlab-agent`). Tools are selected by **capability** after a classifier turn — not injected wholesale.

```
User message
  → LLM #1 IntentRouter (text only) → { capability, confidence, reason }
  → CapabilityToolLoader
  → LLM #2 Yazg (history + that capability’s tools only)
  → optional tool call → response
```

Classifier input is **only the latest user message**. Greeting/knowledge → zero tools (cannot call `analyze_endpoint`). Classifier failure / bad JSON → **Conversation**.

| Capability | Tools | Calling |
|------------|-------|---------|
| Conversation / Knowledge | none | off |
| Workspace | `list_workspace` | on |
| Projects | + `project_detail`, `create_project` | on |
| Targets | `list_targets`, `target_detail`, `analyze_endpoint` | on |
| Scan / Findings / Reports | `list_*`, `*_detail` | on |
| Attack | `attack_plan`, `generate_prompt`, `recommend`, `summary`, `judge` | on |
| Models / Runtime / Settings | reserved empty | off |

`CapabilityRegistry::register` for plugins/MCP. Code: `crates/promptlab-agent/src/assistant/` + `yazg_runtime.rs` (`run_yazg`).

IPC: `yazg_chat`, `yazg_stop`, `yazg_generate_chat_title`, `yazg_resolve_hilt`. Wizard planning also uses Yazg (`planner_generate_from_profile`) — [ATTACK.md](ATTACK.md).

### HILT (human-in-the-loop)

Mutating tools (`create_project`, `create_*` / `update_*` / `delete_*`) do **not** write immediately. Host stores `HiltPendingAction` (TTL **15 min**). Chat shows Approve / Deny; `yazg_resolve_hilt` applies or discards, then a follow-up LLM turn. Expired/denied → no write.

### Memory

STM (`agent_short_term_memory`, session-scoped) and LTM (`agent_long_term_memory`, keyed upsert). IPC: `agent_memory_list_sessions/events/ltm`, `agent_memory_delete_session`. Insights may be promoted STM → LTM after a turn.

Scan execution may pick **SequentialAttackExecutionAgent** vs **AgenticAttackExecutionAgent**, with **ReflectionAgent**, **JudgeCoordinator**, and endpoint **pacing/recovery** after transport failures.

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
