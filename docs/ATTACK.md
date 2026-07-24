# PromptLab Attack Framework

**Crate:** `promptlab-attack`  
**Purpose:** Trait-based AI security attack execution with payload mutation, orchestration, and result collection.

---

## Attack Categories

| Category | ID | Description |
|----------|-----|-------------|
| Prompt Injection | `prompt_injection` | Instruction override, delimiter escape |
| System Prompt Extraction | `system_prompt_extraction` | Hidden policy / system message disclosure |
| Jailbreak | `jailbreak` | Safety guardrail bypass via roleplay |
| RAG Leakage | `rag_leakage` | Retrieved context and source metadata exfil |
| Memory Poisoning | `memory_poisoning` | Persistent malicious memory injection |
| Cross User Leakage | `cross_user_leakage` | Other users' session / tenant data access |
| Agent Goal Hijacking | `agent_goal_hijacking` | Planner / mission objective redirection |
| Tool Abuse | `tool_abuse` | Unauthorized tool calls and parameter injection |
| MCP Abuse | `mcp_abuse` | MCP JSON-RPC tool/resource abuse |

---

## Architecture

```
AttackRegistry
    └── Attack trait (plan, payloads, evaluate)
AttackExecutor
    ├── lifecycle: Planning → Preparing → Executing → Evaluating → Collecting → Completed
    ├── PayloadMutator (7 strategies)
    └── PayloadRunner → TargetTransport (HTTP / mock)
AttackOrchestrator
    └── multi-category runs with probe isolation
ResultCollector
    └── in-memory + optional ResultSink
```

### Attack Lifecycle

| Phase | Action |
|-------|--------|
| `Planning` | Attack produces `AttackPlan` (mutators, payload filter) |
| `Preparing` | Default payloads selected, mutation variants generated |
| `Executing` | Payload delivered via transport |
| `Evaluating` | Response scored for vulnerability indicators |
| `Collecting` | Best result and attempts aggregated |
| `Completed` | Terminal success state |

---

## Usage

```rust
use promptlab_attack::{
    AttackCategory, AttackContext, AttackExecutor, AttackOrchestrator,
    AttackRegistry, HttpTransport, OrchestratorConfig, ResultCollector,
};
use promptlab_attack::types::AttackTarget;

#[tokio::main]
async fn main() -> promptlab_attack::AttackResult<()> {
    let transport = HttpTransport::new();
    let executor = AttackExecutor::new(AttackRegistry::with_builtins(), transport);

    let ctx = AttackContext::new(
        "scan-001",
        "probe-001",
        AttackTarget::llm_api("https://target.example/v1/chat/completions")
            .with_auth("sk-test"),
    );

    // Single attack
    let result = executor
        .execute_category(AttackCategory::PromptInjection, &ctx)
        .await?;

    // Full orchestration
    let orchestrator = AttackOrchestrator::new(executor, OrchestratorConfig::default());
    let report = orchestrator.run(&ctx).await?;

    let collector = ResultCollector::new();
    collector.collect_orchestration(report).await?;

    Ok(())
}
```

---

## Payload Mutators

| Mutator | Effect |
|---------|--------|
| `Base64Wrap` | Encodes payload in base64 instruction wrapper |
| `UnicodeHomoglyph` | Cyrillic homoglyph substitution |
| `DelimiterInjection` | Fake system delimiter blocks |
| `RoleSwap` | Chat role confusion |
| `ChunkSplit` | Split-and-recombine payload |
| `JsonEscape` | JSON string escaping |
| `RepeatAmplify` | Duplicate emphasis |

---

## Confidence & Severity

Each payload attempt produces an `AttackEvaluation`:

- `success` — vulnerability indicators matched
- `confidence` — heuristic score (0.0–1.0)
- `severity` — `Info` / `Low` / `Medium` / `High` / `Critical`
- `indicators` — matched signal labels

---

## Tests

```bash
cargo test -p promptlab-attack
```

Coverage: lifecycle transitions, all 9 registry entries, mutators, executor, orchestrator, result collector, mock transport.

---

## Storage Integration

Attack results map to `attack_results` and `findings` tables in `promptlab-storage`. Enable the optional `storage` feature for persistence sinks (future).
