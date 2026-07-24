# Attack Engine Status

**Crate:** `promptlab-attack` v0.1.0  
**Companion:** `promptlab-payload` (mutation helpers only)  
**Date:** 2026-06-10  
**Classification:** **Partial implementation**

---

## Verdict

| Classification | Applies? | Rationale |
|----------------|----------|-----------|
| **1. Real implementation** | Partially | Executor, transport, 9 attack plugins, and lifecycle are functional library code |
| **2. Partial implementation** | **Yes** | Core pipeline works; config gaps, no HTTP tests, no app integration, lib tests broken |
| **3. Skeleton** | No | ~2,600 LOC, full trait pipeline, 9 categories with payloads and evaluators |

`promptlab-attack` is a **working attack framework library**, not a skeleton. It is **not production-complete** because orchestration config is partially ignored, results are not auto-persisted, evaluation is heuristic-only (no judge), and all automated tests use `MockTransport` instead of live HTTP.

---

## Capability Verification

### Summary matrix

| Capability | Status | Confidence | Evidence |
|------------|--------|------------|----------|
| Payload execution | **Real** | High | `PayloadRunner` → `TargetTransport::send()` |
| Request generation | **Real** | High | `build_request()` with `{{payload}}` template, headers, auth |
| Response collection | **Partial** | High | `ResultCollector` exists; not wired into executor/orchestrator |
| Attack orchestration | **Partial** | High | Sequential multi-category runs; `concurrency` unused |

---

## 1. Payload Execution

**Status: Real implementation**

### What exists (real)

| Component | File | Behavior |
|-----------|------|----------|
| Payload runner | `payload/runner.rs` | Builds request, sends via transport, returns `AttackResponse` |
| HTTP transport | `transport/http.rs` | `reqwest` client — method, headers, body, timeout, status/body capture |
| Mock transport | `transport/mock.rs` | Test double — captures requests, returns canned responses |
| Transport trait | `transport/mod.rs` | `TargetTransport::send(TransportRequest)` |
| Mutator pipeline | `payload/mutator.rs` | 7 mutation strategies before each send |
| Budget: payloads | `executor.rs` | `ctx.budget.max_payloads` enforced in execution loop |
| Budget: timeout | `payload/runner.rs` | `ctx.budget.timeout_ms` passed to transport |

### Execution flow

```
AttackExecutor::execute_attack()
  → select_payloads()
  → mutator.expand(content, plan.mutators)
  → PayloadRunner::execute(ctx, payload, mutated_content)
       → build_request()
       → transport.send()
  → attack.evaluate(ctx, payload, response)
  → PayloadAttempt { response, evaluation, ... }
```

### Default production path

```rust
// lib.rs
pub fn default_executor() -> AttackExecutor<HttpTransport> {
    AttackExecutor::new(default_registry(), HttpTransport::new())
}
```

Production code uses **`HttpTransport`** (reqwest). **`MockTransport`** is test-only.

### Gaps / defects

| Issue | Severity | Detail |
|-------|----------|--------|
| No HTTP integration tests | Medium | `wiremock` in dev-deps but **unused**; no live HTTP test coverage |
| `PayloadFormat` ignored | Medium | `Plain` only; `JsonTemplate`, `MultiTurn` defined but not handled |
| MCP JSON payloads | Low | Raw JSON-RPC strings injected via same `{{payload}}` LLM chat template — wrong shape for MCP targets |
| Lib unit test compile error | Low | `payload/runner.rs` test: `PayloadRunner::new(transport)` missing `&` |

### Classification rationale

Payload delivery through HTTP is **real**. Gaps are format routing and test coverage, not missing execution logic.

---

## 2. Request Generation

**Status: Real implementation**

### What exists

| Function | File | Behavior |
|----------|------|----------|
| `build_request()` | `payload/runner.rs` | Assembles `TransportRequest` from `AttackContext` + payload content |
| `AttackTarget::llm_api()` | `types.rs` | Default OpenAI-style JSON: `{"model":"gpt-4o","messages":[{"role":"user","content":"{{payload}}"}]}` |
| Template injection | `payload/runner.rs` | `body_template.replace("{{payload}}", payload_content)` |
| Auth header | `payload/runner.rs` | `Authorization: Bearer {token}` if `auth_token` set |
| Content-Type default | `payload/runner.rs` | `application/json` if not provided |
| Method override | `types.rs` | `method: Option<String>`, defaults to `POST` |
| Custom headers | `types.rs` | `AttackTarget::with_header()` |

### Verified behavior

- Unit test `builds_json_body_from_template` — passes when lib tests compile (uses `MockTransport`, verifies status 200)
- `MockTransport::captured_requests()` records full request for assertion (test infrastructure)

### What is NOT implemented

- Per-attack or per-payload body templates (only target-level `body_template`)
- GraphQL, form-encoded, or multipart request builders
- Multi-turn conversation assembly (`PayloadFormat::MultiTurn`)
- Streaming request/response (SSE, chunked)
- Cookie jar / session replay
- Target-specific routing by `TargetKind` (LlmApi, Agent, Rag, Mcp — enum exists, runner ignores it)

### Classification rationale

Request generation for **LLM chat API POST with JSON template** is **real and complete for that shape**. Broader target types and formats are **missing**.

---

## 3. Response Collection

**Status: Partial implementation**

### What exists (real)

| Component | File | Behavior |
|-----------|------|----------|
| `AttackResponse` | `types.rs` | `status`, `headers`, `body`, `duration_ms` |
| `PayloadAttempt` | `types.rs` | Full record: payload, mutation, response, evaluation |
| `AttackExecutionResult` | `types.rs` | All attempts + `best` evaluation + lifecycle phase |
| `OrchestrationReport` | `types.rs` | Aggregated multi-attack results + `findings_count` |
| `ResultCollector` | `collector.rs` | In-memory store for executions and orchestrations |
| `ResultSink` trait | `collector.rs` | Async hook for external persistence |
| `successful_findings()` | `collector.rs` | Filters positive evaluations with severity |

### What exists (evaluation / parsing)

| Function | File | Behavior |
|----------|------|----------|
| `extract_response_text()` | `attacks/common.rs` | Parses OpenAI, Anthropic, Gemini, generic JSON shapes |
| `matching_indicators()` | `attacks/common.rs` | Regex-based vulnerability signal detection |
| Per-attack `evaluate()` | `attacks/*.rs` | Heuristic scoring → `AttackEvaluation` |

### Gaps

| Issue | Severity | Detail |
|-------|----------|--------|
| **Collector not auto-wired** | High | `AttackExecutor` and `AttackOrchestrator` do not call `ResultCollector` |
| **No storage sink impl** | High | `ResultSink` trait defined; no `promptlab-storage` adapter despite optional dep |
| **`storage` feature unused** | Medium | `Cargo.toml` declares `storage = ["dep:promptlab-storage"]` — no `#[cfg(feature = "storage")]` code |
| **No judge integration** | High | Heuristic eval only; `promptlab-judge` not called from executor |
| Lifecycle `Collecting` phase | Low | Set in `lifecycle.complete()` but no I/O during it |
| Response size limits | Low | No cap on body size (unlike discovery's 2 MB limit) |

### Manual collection pattern (current)

Integration test shows the expected usage — caller must explicitly collect:

```rust
let report = orchestrator.run(&ctx).await?;
let collector = ResultCollector::new();
collector.collect_orchestration(report).await?;
```

### Classification rationale

Response **capture** and **in-memory aggregation** are **real**. **Automatic collection, persistence, and judge-backed evaluation** are **missing** → overall **partial**.

---

## 4. Attack Orchestration

**Status: Partial implementation**

### What exists (real)

| Component | File | Behavior |
|-----------|------|----------|
| `AttackOrchestrator` | `orchestrator.rs` | Runs multiple `AttackCategory` values against one `AttackContext` |
| `AttackExecutor` | `executor.rs` | Single-attack lifecycle: plan → prepare → execute → evaluate → complete |
| `AttackRegistry` | `registry.rs` | 9 built-in attacks keyed by id and category |
| `AttackLifecycle` | `lifecycle.rs` | Phase machine with transition validation |
| `OrchestratorConfig` | `orchestrator.rs` | Categories list, `stop_on_first_critical` |
| Probe isolation | `orchestrator.rs` | Unique `probe_id` per category (`{probe_id}-{category}`) |
| Error handling | `orchestrator.rs` | Failed category → `AttackPhase::Failed` result in report (does not abort run) |

### Built-in attack categories (all real implementations)

| Category | File | Payloads | Evaluator |
|----------|------|----------|-----------|
| Prompt Injection | `prompt_injection.rs` | 3 | Regex indicators |
| System Prompt Extraction | `system_prompt_extraction.rs` | 3 | Regex indicators |
| Jailbreak | `jailbreak.rs` | 3 | Refusal + bypass indicators |
| RAG Leakage | `rag_leakage.rs` | 3 | Source/chunk/citation patterns |
| Memory Poisoning | `memory_poisoning.rs` | 3 | Persistence/compliance patterns |
| Cross User Leakage | `cross_user_leakage.rs` | 3 | Tenant/session leak patterns |
| Agent Goal Hijacking | `agent_goal_hijacking.rs` | 3 | Mission/planner override patterns |
| Tool Abuse | `tool_abuse.rs` | 3 | Tool call JSON + command output |
| MCP Abuse | `mcp_abuse.rs` | 3 | JSON-RPC / file read patterns |

Each attack implements the full `Attack` trait: `plan()`, `default_payloads()`, `evaluate()`.

### Config fields declared but NOT enforced

| Field | Location | Actual behavior |
|-------|----------|-----------------|
| `OrchestratorConfig.concurrency` | `orchestrator.rs` | **Ignored** — always sequential `for` loop |
| `AttackBudget.max_mutations_per_payload` | `types.rs` | **Ignored** — mutator uses hardcoded `MutatorConfig.max_per_payload: 3` |
| `stop_on_first_critical` | `orchestrator.rs` | **Works** — breaks loop on critical severity |

### Gaps

| Issue | Severity | Detail |
|-------|----------|--------|
| No parallel execution | Medium | `concurrency` field is dead code |
| No scan-level DAG / resume | Medium | Single-context sequential runs only |
| No cancellation / abort token | Medium | `AttackPhase::Cancelled` exists, no API to trigger |
| No progress callbacks | Medium | `tracing` logs only |
| No Tauri IPC | High | Not wired to desktop app |
| No discovery → attack handoff | High | Endpoints from discovery not fed as targets |

### Classification rationale

Multi-category **sequential orchestration** with lifecycle and error isolation is **real**. Parallelism, persistence, and scan pipeline integration are **missing** → **partial**.

---

## Real vs Mock vs Missing

### Real code (production library)

| Area | Files | Notes |
|------|-------|-------|
| Attack trait + 9 plugins | `traits.rs`, `attacks/*.rs` | Full plan/payload/evaluate per category |
| Executor lifecycle | `executor.rs`, `lifecycle.rs` | End-to-end single-attack pipeline |
| Orchestrator | `orchestrator.rs` | Multi-category sequential runs |
| Request builder | `payload/runner.rs` | Template injection, headers, timeout |
| HTTP transport | `transport/http.rs` | reqwest-based production transport |
| Payload mutator | `payload/mutator.rs` | 7 strategies; uses `promptlab_payload::{base64_encode, unicode_obfuscate}` |
| Registry | `registry.rs` | Builtin registration, lookup by id/category |
| Response parsing | `attacks/common.rs` | Multi-vendor LLM JSON extraction |
| Heuristic evaluation | `attacks/*.rs` | Regex + keyword scoring per category |
| Result types + collector | `types.rs`, `collector.rs` | Structured results; manual collection |
| Error model | `error.rs` | Typed `AttackError` variants |

### Mock code (test / dev only)

| Component | File | Purpose | Production replacement |
|-----------|------|---------|------------------------|
| `MockTransport` | `transport/mock.rs` | Returns canned HTTP responses; captures requests | `HttpTransport` |
| All unit/integration tests | `*_test`, `tests/integration.rs` | Use `MockTransport::ok(...)` | `HttpTransport` + wiremock or live target |
| Heuristic evaluators | `attacks/*.rs` | Regex/keyword scoring (not LLM) | Optional: `promptlab-judge` consensus (not wired) |

**Note:** Built-in attack **payload strings** (e.g. DAN jailbreak, MCP JSON-RPC) are **real attack content**, not mocks. They are static defaults, not generated fakes.

### Missing code

| Feature | Expected location | Status |
|---------|-------------------|--------|
| `StorageResultSink` (persist to SQLite) | `collector.rs` or `storage/` module | ❌ Trait only, no impl |
| Judge-backed evaluation | `executor.rs` post-eval hook | ❌ Not integrated |
| `OrchestratorConfig.concurrency` worker pool | `orchestrator.rs` | ❌ Field unused |
| `AttackBudget.max_mutations_per_payload` wiring | `executor.rs` | ❌ Field unused |
| `PayloadFormat::JsonTemplate` / `MultiTurn` | `payload/runner.rs` | ❌ Enum unused beyond default |
| `TargetKind`-aware request routing | `payload/runner.rs` | ❌ All targets use same template path |
| `promptlab-payload` library integration | `payload/` or `attacks/` | ❌ Only 2 encoding functions used; `payloads.json` not loaded |
| HTTP integration tests | `tests/` | ❌ `wiremock` declared, unused |
| Progress / cancellation API | `orchestrator.rs`, `executor.rs` | ❌ |
| Tauri IPC commands | `src-tauri/` | ❌ |
| Discovery → attack target mapping | Pipeline layer | ❌ |
| Auth session transport | `transport/` | ❌ Bearer token only |

---

## Architecture Overview

```
AttackOrchestrator::run(ctx)
    │
    └─ for each category in config.categories
          └─ AttackExecutor::execute_category(category, ctx)
                │
                ├─ AttackLifecycle: Planning
                │     └─ attack.plan(ctx) → AttackPlan
                │
                ├─ Preparing
                │     └─ select_payloads() + mutator.expand()
                │
                ├─ Executing (loop)
                │     └─ PayloadRunner::execute()
                │           └─ build_request() → HttpTransport::send()  [REAL]
                │
                ├─ Evaluating (loop)
                │     └─ attack.evaluate() → heuristic regex scoring  [REAL, not judge]
                │
                └─ Collecting → Completed
                      └─ AttackExecutionResult (in-memory only)

ResultCollector::collect_orchestration()  ← MANUAL, not called by orchestrator
ResultSink                                  ← TRAIT ONLY, no storage impl
```

---

## Module Inventory

| Module | ~Lines | Role | Maturity |
|--------|--------|------|----------|
| `executor.rs` | 198 | Single-attack pipeline | Real |
| `orchestrator.rs` | 141 | Multi-category runs | Partial |
| `payload/runner.rs` | 101 | Request build + send | Real |
| `payload/mutator.rs` | 104 | 7 mutation strategies | Real |
| `transport/http.rs` | 85 | reqwest transport | Real |
| `transport/mock.rs` | 53 | Test transport | Mock |
| `collector.rs` | 134 | In-memory results | Partial |
| `registry.rs` | 82 | Attack registry | Real |
| `lifecycle.rs` | 151 | Phase machine | Real |
| `types.rs` | 256 | Domain model | Real (some fields unused) |
| `attacks/*.rs` | ~900 | 9 attack plugins | Real (heuristic eval) |
| `attacks/common.rs` | 85 | Response parsing helpers | Real |

**Total:** 29 files, ~2,624 LOC — not a skeleton.

---

## Dependency Usage

| Dependency | Declared | Actually used |
|------------|----------|---------------|
| `reqwest` | ✅ | `HttpTransport` |
| `promptlab-payload` | ✅ | `base64_encode`, `unicode_obfuscate` in mutator only |
| `promptlab-core` | ✅ | Error re-export |
| `promptlab-storage` | Optional feature | **Not referenced in code** |
| `wiremock` | dev-dep | **Not referenced in tests** |
| `regex`, `serde_json` | ✅ | Evaluators, JSON parsing |
| `tracing` | ✅ | Executor/orchestrator instrumentation |

---

## Test Status

| Suite | Result | Notes |
|-------|--------|-------|
| Lib unit tests (`--lib`) | ❌ **Compile fail** | `PayloadRunner::new(transport)` → needs `&transport` in `payload/runner.rs:91` |
| Integration `full_orchestration_with_collector` | ✅ Pass | MockTransport; 2 categories |
| Integration `all_categories_execute_without_error` | ✅ Pass | All 9 categories complete with MockTransport |
| Embedded unit tests (if lib compiled) | ~12 tests | lifecycle, mutator, registry, common, collector, executor, orchestrator |
| HTTP / wiremock tests | ❌ None | No live transport verification |

```bash
cargo test -p promptlab-attack --test integration   # 2/2 pass
cargo test -p promptlab-attack --lib                  # compile error
```

---

## Integration Status

| Layer | Status |
|-------|--------|
| Library API (`default_executor()`, `AttackOrchestrator`) | ✅ Callable |
| `HttpTransport` to real LLM API | ✅ Implemented, untested in CI |
| `promptlab-judge` evaluation | ❌ Not wired |
| `promptlab-storage` persistence | ❌ Feature stub only |
| `promptlab-payload` library payloads | ❌ Not loaded |
| Tauri IPC | ❌ Not wired |
| UI Attacks page | ❌ Mock data (`src/shared/mock/data.ts`) |

---

## Comparison to Skeleton

A skeleton would expose trait definitions with `todo!()` bodies or return hardcoded findings. This crate has:

- Working executor loop with mutation expansion and budget checks
- Real reqwest HTTP client
- 9 attack modules with distinct payloads and evaluators
- Lifecycle state machine with transition guards
- Structured result types suitable for persistence
- Integration tests exercising all 9 categories end-to-end (via mock transport)

**Conclusion:** Not a skeleton.

---

## Comparison to Full / Real Implementation

Missing for production-ready attack engine:

| Feature | Status |
|---------|--------|
| Fix lib test compile error | ❌ |
| HTTP integration tests (wiremock) | ❌ |
| Honor `concurrency` and `max_mutations_per_payload` | ❌ |
| Auto-wire `ResultCollector` in orchestrator | ❌ |
| `StorageResultSink` via `promptlab-storage` | ❌ |
| Judge consensus instead of/in addition to heuristics | ❌ |
| TargetKind-specific request builders | ❌ |
| Multi-turn / MCP-native transport | ❌ |
| Cancellation and progress streaming | ❌ |
| Tauri `scan_run` pipeline integration | ❌ |

---

## Final Classification

```
promptlab-attack
├── Overall ...................... PARTIAL IMPLEMENTATION
├── Payload execution ............ REAL (HttpTransport + PayloadRunner)
├── Request generation ........... REAL (LLM JSON template path)
├── Response collection .......... PARTIAL (types real; auto-persist missing)
├── Attack orchestration ......... PARTIAL (sequential only; config gaps)
├── Attack plugins (×9) ............ REAL (heuristic eval, static payloads)
├── Mock code .................... MockTransport + all test harnesses
└── Missing ...................... storage sink, judge, concurrency, IPC
```

**Recommendation:** Use `AttackExecutor` + `HttpTransport` for MVP Step 4 (prompt injection against discovered LLM endpoint). Wire results into `ResultCollector` manually in Tauri `scan_run`. Fix lib test borrow, add wiremock HTTP test, and connect `promptlab-judge` before marking COMPLETE.

---

*Related: [ATTACK.md](ATTACK.md), [PAYLOAD.md](PAYLOAD.md), [MVP_GAP_ANALYSIS.md](MVP_GAP_ANALYSIS.md), [MOCK_INVENTORY.md](MOCK_INVENTORY.md), [STATUS.md](STATUS.md)*
