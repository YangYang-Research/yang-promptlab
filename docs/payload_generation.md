# AISec Payload Generator

Dynamic prompt payload generation from attack plans, feeding the attack engine during scans.

## Pipeline

```
AttackPlan  →  Payload Generator  →  PromptPayloads  →  AttackExecutor
```

Upstream planner output (`aisec-planner::AttackPlan`) selects categories and disabled tests. The generator materializes `AttackPayload` probes per category and injects them into `AttackContext.generated_payloads` for execution.

## Crate: `aisec-generator`

| Module | Role |
|--------|------|
| `types` | `PromptPayloads`, `GeneratorMode`, `GeneratorStats` |
| `static_pack` | Built-in catalog from `aisec-payload` (no mutations) |
| `template_mutation` | Catalog + encoding/template mutations via `PayloadPipeline` |
| `local_llm` | Vault LLM synthesizes novel probes per category |
| `convert` | `PayloadRecord` / `GeneratedPayload` → `AttackPayload` |
| `engine` | `generate_prompt_payloads()` entry point |

## Input: `AttackPlan`

Uses the planner-level plan (not per-attack executor plans):

```rust
pub struct AttackPlan {
    pub profile_id: String,
    pub categories: Vec<AttackCategory>,
    pub disabled_tests: Vec<String>,
    // ...
}
```

## Output: `PromptPayloads`

```rust
pub struct PromptPayloads {
    pub mode: GeneratorMode,
    pub by_category: HashMap<AttackCategory, Vec<AttackPayload>>,
    pub payload_ids: Vec<String>,
    pub stats: GeneratorStats,
    pub summary: String,
    pub llm_note: Option<String>,
}
```

## Supported categories

User-facing labels map to `AttackCategory` / `aisec-payload` catalog entries:

| Label | Engine category | Catalog |
|-------|-----------------|---------|
| Prompt Injection | `prompt_injection` | `pi-*` |
| System Prompt Extraction | `system_prompt_extraction` | `spe-*` |
| Tool Abuse | `tool_abuse` | `ta-*` |
| Memory Poisoning | `memory_poisoning` | `mp-*` |
| Data Exfiltration | `cross_user_leakage` | `cul-*` |
| Agent Hijacking | `agent_goal_hijacking` | `agh-*` |
| RAG Poisoning | `rag_leakage` | `rag-*` |

Additional engine categories (`jailbreak`, `mcp_abuse`) are included when selected by the attack plan.

## Generation modes

| Mode | Behavior |
|------|----------|
| **Static Pack** | Loads `crates/aisec-payload/data/payloads.json`; filters by plan categories and `disabled_tests` |
| **Template Mutation** | Static sources expanded with `MutationKind` encodings (base64, hex, HTML, unicode obfuscation) |
| **Local LLM** | Static baseline + 2–3 LLM probes per category; falls back to static-only if parse fails |

Local LLM mode uses the same vault model bridge as the judge and planner (`ModelProviderRuntime` + `LocalLlmBackend`).

## Attack engine integration

`AttackContext` carries optional generated payloads:

```rust
pub generated_payloads: Option<HashMap<AttackCategory, Vec<AttackPayload>>>,
```

`AttackExecutor` prefers `generated_payloads` for the active category before falling back to attack module builtins.

Scans pass `generator_mode` in the playbook. At job start, `scan_start` invokes the generator once and threads the result through every `run_category_on_endpoint` call.

## IPC

```typescript
generatePromptPayloads({
  profileId: string,
  categories: string[],
  disabledTests: string[],
  mode: "static_pack" | "template_mutation" | "local_llm",
})
```

Command: `generator_generate`

Scan start accepts `generatorMode` (default: `static_pack`):

```typescript
startScan({
  // ...
  generatorMode: "template_mutation",
})
```

## Scan Wizard (Step 4)

After attack planning, use **Payload Generator** buttons:

- **Static Pack** — preview catalog probes
- **Template Mutation** — preview encoded variants
- **Local LLM** — preview LLM-augmented set (requires vault model)

Selected `generatorMode` is sent with `scan_start` and applied to all endpoints in the scan job.

## Example

**Plan:** OpenWebUI + tools + memory → Prompt Injection, Tool Abuse, Memory Poisoning

**Static pack output:**

- `pi-direct-override`, `pi-indirect-task`, …
- `ta-shell`, `ta-path-traversal`
- `mp-persist-instruction`, `mp-false-fact`

**Template mutation** adds base64/hex/HTML/unicode variants per source (up to 4 per payload by default).

## Related docs

- [attack_planner.md](./attack_planner.md) — upstream planning
- [fingerprint_engine.md](./fingerprint_engine.md) — platform identification
- [PAYLOAD.md](./PAYLOAD.md) — static catalog and mutation engine
