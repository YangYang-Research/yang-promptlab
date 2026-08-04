# AgentTrace

AgentTrace is PromptLab’s GenAI span-tracing library (Rust) used by the Agent Trace UI.

## Concepts

| AgentTrace | Meaning |
|------------|---------|
| Experiment (`yazg`) | Named experiment bucket |
| Trace | One Yazg turn |
| Span | Step inside a turn (`capability_classify`, `llm`, `tool:*`) |
| `session_id` | Conversation grouping (`yazg-chat:<threadId>`) |

## Storage

SQLite file: `~/.promptlab/agenttrace/agenttrace.db`

Tables: `at_experiments`, `at_traces`, `at_spans`.

## Crate

`crates/promptlab-agenttrace` — `AgentTrace::open`, `experiment`, `start_trace`,
`span`, `end`, plus query APIs used by desktop IPC.

## Instrumentation

`run_yazg` starts a trace per turn, records:

1. `capability_classify` span (inputs = classifier request, outputs = capability JSON)
2. `llm` span per completion (wire request/response)
3. `tool:<name>` spans for tool calls

`agents.log` is still dual-written for debugging; **Agent Trace UI reads AgentTrace only**.

LLM spans record `input_tokens` / `output_tokens` / `total_tokens` in span
metrics (provider usage when available, otherwise a char/4 estimate). List/detail
APIs expose the sum as `totalTokens` on each trace.

## IPC

- `agenttrace_list_sessions`
- `agenttrace_list_traces`
- `agenttrace_get_trace`
- `agenttrace_delete_session`

Deleting an Assistant conversation also deletes AgentTrace rows for that session.
