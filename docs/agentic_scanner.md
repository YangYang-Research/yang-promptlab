# AISec Agentic Scanner

Autonomous attack execution with a closed-loop pipeline: fingerprint, plan, attack, judge, and retry until a vulnerability is found or the attempt budget is exhausted.

## Pipeline

```
Fingerprint  →  Plan  →  Generate  →  Attack  →  Judge  →  Retry?
                  ↑__________________________________________|
```

Stop conditions:

- **Vulnerability found** — judge returns `vulnerable: true` for any attempt
- **Max attempts reached** — per-category retry budget exhausted (default: 5)
- **Cancelled** — user stops the scan from the Scans page

## Crate: `aisec-agent`

| Module | Role |
|--------|------|
| `types` | `AgentConfig`, `AgentPhase`, `CategoryAgentResult`, `AgentScanResult` |
| `host` | `AgentHost` trait — attack/judge/persistence bridge |
| `plan` | Planner integration + category intersection |
| `retry` | Retry policy + generator mode escalation |
| `engine` | `run_category_episode`, `run_endpoint_agent` |

## Capabilities

| Capability | Implementation |
|------------|----------------|
| **Plan** | `aisec-planner::generate_attack_plan` from stored endpoint fingerprint |
| **Execute** | `aisec-attack` via `run_category_on_endpoint` |
| **Evaluate** | `aisec-judge` + plugin judge signals |
| **Retry** | `should_retry()` when judge reports not vulnerable |
| **Mutate payloads** | Escalate `GeneratorMode`: static → template mutation → local LLM |

## Configuration

```rust
pub struct AgentConfig {
    pub max_attempts_per_category: usize,  // default 5
    pub planner_mode: PlannerMode,         // Deterministic
    pub initial_generator_mode: GeneratorMode,
}
```

Retry escalation (`retry.rs`):

| Retry | Generator mode |
|-------|----------------|
| 0 | Initial mode (from scan `generator_mode`) |
| 1 | `TemplateMutation` |
| 2+ | `LocalLlm` |

## Tauri integration

`src-tauri/src/agent_service.rs` implements `AgentHost` for background scans.

When `agent_mode: true` in the scan playbook:

1. `scan_start` spawns `run_agent_scan_job` instead of `run_scan_job`
2. Scan name: `Agent Scan ({profile})`
3. Progress includes `agent_mode`, `current_phase`, `current_attempt`, `current_retry`

### Playbook fields

```json
{
  "profile": "standard",
  "categories": ["prompt_injection", "tool_abuse"],
  "disabled_tests": [],
  "endpoint_ids": ["..."],
  "generator_mode": "static_pack",
  "agent_mode": true,
  "max_agent_attempts": 5
}
```

## IPC

```typescript
startScan({
  projectId, targetId, endpointIds,
  categories, profile, disabledTests,
  generatorMode: "static_pack",
  agentMode: true,
  maxAgentAttempts: 5,
})
```

## Scan Wizard (Step 4)

**Agentic Scanner** section:

- **Enable agentic execution** — toggles `agentMode`
- **Max attempts per category** — retry budget (1–20)

## Scans page

- Agent scans appear as `Agent Scan (profile)` in the list
- Active agent scans show an **Agent** badge
- Live status displays agent phase, attempt, and retry count

## Example episode

1. **Fingerprint** — load `endpoints.fingerprint_json`
2. **Plan** — OpenWebUI + tools → Prompt Injection, Tool Abuse, …
3. **Generate** — static pack probes for Prompt Injection
4. **Attack** — send probes via harness transport
5. **Judge** — no vulnerability → **Retry** with template mutation
6. **Attack** + **Judge** again → vulnerability found → stop category

## Related docs

- [attack_planner.md](./attack_planner.md)
- [payload_generation.md](./payload_generation.md)
- [fingerprint_engine.md](./fingerprint_engine.md)
