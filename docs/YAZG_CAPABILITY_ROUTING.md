# Yazg Capability-Based Tool Routing

## Goal

Stop injecting every Yazg tool into every LLM request. Tools are selected by **capability** after an LLM capability classifier.

## Flow (two LLM turns)

```text
User message
    ↓
LLM #1  IntentRouter          (text-only, no tools)
        → JSON { capability, confidence, reason }
    ↓
CapabilityToolLoader          (registry lookup)
    ↓
LLM #2  Yazg agent            (system + history + capability tools only)
    ↓
Optional tool call
    ↓
Response
```

Greeting / knowledge → capability with **zero tools** → LLM #2 cannot call `analyze_endpoint`.

Classifier input is **only the latest user message** — no conversation history.

## Capabilities

| Capability   | Tools (current Yazg)                                      | Tool calling |
|-------------|------------------------------------------------------------|--------------|
| Conversation | _(none)_                                                  | forced off   |
| Knowledge    | _(none)_                                                  | forced off   |
| Workspace    | `list_workspace`                                          | on           |
| Projects     | `list_workspace`, `project_detail`, `create_project`      | on           |
| Targets      | `list_targets`, `target_detail`, `analyze_endpoint`       | on           |
| Scan         | `list_scan`, `scan_detail`                                | on           |
| Findings     | `list_findings`, `finding_detail`                         | on           |
| Reports      | `list_reports`, `report_detail`                           | on           |
| Attack       | `attack_plan`, `generate_prompt`, `recommend`, `summary`, `judge` | on |
| Models / Runtime / Settings | _(reserved, empty until wired)_               | off          |

The registry is extensible (`CapabilityRegistry::register`) for plugins / MCP without changing AI Runtime.

## Code

| Piece | Path |
|-------|------|
| Registry | `crates/promptlab-agent/src/assistant/capability_registry.rs` |
| LLM router | `crates/promptlab-agent/src/assistant/router/mod.rs` |
| Loader | `crates/promptlab-agent/src/assistant/capability_loader.rs` |
| Wiring | `crates/promptlab-agent/src/yazg_runtime.rs` (`run_yazg`) |

## Logging / Agent Trace

Agent Trace shows a **stage timeline** from `agents.log` plus STM from SQLite:

- `capability_classify_request` / `capability_classify_response` (LLM #1)
- `llm_request` / `llm_response` (LLM #2, capability tools only)
- `completion_*` / `tool_*` when present
- Info: capability, loaded tool count/names, tool_used, latency, `router=llm`

Deleting a conversation in Assistant also deletes STM (`yazg-chat:<threadId>`). Trace only lists sessions still present in Assistant.

## Fallback

If classifier LLM fails or JSON is unparseable → **Conversation** (0 tools).
