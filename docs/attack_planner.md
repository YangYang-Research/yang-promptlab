# PromptLab Attack Planner

Dynamic attack plan generation from fingerprint results, integrated into Scan Wizard step 4.

## Pipeline

```
FingerprintResult  →  Attack Planner  →  AttackPlan  →  scan_start
```

## Crate: `promptlab-planner`

| Module | Role |
|--------|------|
| `types` | `FingerprintResult`, `AttackPlan`, `PlannerMode` |
| `deterministic` | Rule-based planning from platform capabilities |
| `local_llm` | LLM-refined plans via `PlannerLlm` trait |
| `normalize` | Fingerprint category → `AttackCategory` mapping |
| `engine` | `generate_attack_plan()` entry point |

## Input: `FingerprintResult`

```rust
pub struct FingerprintResult {
    pub endpoints: Vec<FingerprintEndpoint>,
}

pub struct FingerprintEndpoint {
    pub endpoint_id: String,
    pub url: String,
    pub report: StackFingerprintReport,
}
```

## Output: `AttackPlan`

```rust
pub struct AttackPlan {
    pub mode: PlannerMode,           // Deterministic | LocalLlm
    pub profile_id: String,          // quick | standard | deep | custom
    pub categories: Vec<AttackCategory>,
    pub disabled_tests: Vec<String>,
    pub rationales: Vec<CategoryRationale>,
    pub confidence: f32,
    pub summary: String,
    pub llm_rationale: Option<String>,
}
```

## Example (deterministic)

**OpenWebUI + tools + memory** produces:

- Prompt Injection
- Tool Abuse
- Memory Poisoning
- (+ jailbreak, system prompt extraction, cross-user leakage, agent goal hijacking per capability rules)

Summary: `openwebui · capabilities: memory+tools => Prompt Injection, …`

## Modes

| Mode | Behavior |
|------|----------|
| **Deterministic** | Capability matrix + platform rules + fingerprint recommendations |
| **Local LLM** | Sends fingerprint JSON to vault model; parses JSON plan; falls back to deterministic on parse failure |

Local LLM mode uses the same vault model bridge as the judge (`ModelProviderRuntime` + `LocalLlmBackend`).

## IPC

```typescript
generateAttackPlan({
  endpointIds: string[],
  mode: "deterministic" | "local_llm",
})
```

Command: `planner_generate` — loads `endpoints.fingerprint_json`, builds `FingerprintResult`, returns `AttackPlanDto`.

## Scan Wizard (Step 4)

- **Generate (Deterministic)** — calls `planner_generate` with rule-based mode
- **Generate (Local LLM)** — requires configured vault model on Models page
- **Apply rule suggestions** — legacy client-side fingerprint aggregation (fallback)
- Displays plan summary and top rationales

## Related

- [`docs/fingerprint_engine.md`](fingerprint_engine.md) — fingerprint pipeline
- `src/features/scans/steps/AttackPlanStep.tsx` — UI
- `src-tauri/src/commands/planner.rs` — IPC
