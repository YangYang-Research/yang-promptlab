# AISec — Real Implementation Audit

> **Trạng thái (2026-06-12):** Audit tại thời điểm 2026-06-11. Nhiều mục BROKEN/PARTIAL đã được
> sửa qua PR #19. Xem `docs/STATUS.md` và `docs/MVP_VALIDATION_REPORT.md` cho trạng thái hiện tại.

**Auditor role:** Principal Software Architect
**Date:** 2026-06-11
**Repository:** `yang-aisec-private`
**Scope:** All 11 Rust domain crates + the Tauri shell + React desktop UI
**Method:** Full source read of every module, cross-referenced against `docs/*` and verified
against **live `cargo test` / `npm test` runs** executed in this environment (Linux, Rust 1.96,
Node 22). No code was modified to produce this report.

> This document supersedes the point-in-time findings in `docs/AUDIT_REPORT.md` where they
> diverge. Notably, on this Linux toolchain `aisec-models` and `aisec-report` tests **pass**
> (the earlier report's macOS-specific `sysctl` failures do not occur), and `aisec-plugin-host`
> fails **1 of 6** unit tests (not all), with a different root cause than previously recorded.

---

## Classification legend

| Label | Meaning |
|-------|---------|
| **COMPLETE** | Compiles, all tests pass, fully implements its documented scope. Only cosmetic gaps. |
| **PARTIAL** | Compiles and largely works, but has missing features, incomplete paths, or minor test gaps. |
| **SKELETON** | Structure/types/wiring present, but little or no real domain logic. |
| **MOCK** | Looks functional but is driven by fake/hardcoded data; performs no real work. |
| **BROKEN** | Fails to compile, hangs, or fails tests in its intended use. |

---

## Executive summary

| # | Module | Classification | One-line justification |
|---|--------|----------------|------------------------|
| 1 | `aisec-core` | **COMPLETE** | Small, working error + logging foundation; both unit tests pass; only an ignored `json_file` flag. |
| 2 | `aisec-storage` | **BROKEN** | Production SQLite library is real and complete-grade, but `cargo test` fails to compile (missing trait imports in 2 test modules). |
| 3 | `aisec-discovery` | **BROKEN** | Detectors/HTTP/single-worker crawl are real, but the default multi-worker crawler deadlocks and a test hangs indefinitely. |
| 4 | `aisec-auth` | **BROKEN** | Real Playwright subprocess + session design, but the crate does not compile (`tokio` `process`/`fs`/`io-util` features not enabled). |
| 5 | `aisec-fingerprint` | **COMPLETE** | 59 real provider rules across 8 providers, documented scoring/penalties, all 15 tests pass. |
| 6 | `aisec-payload` | **COMPLETE** | Real 24-entry payload library + real encoders + working pipeline; all 15 tests pass (optional `storage` feature unimplemented). |
| 7 | `aisec-attack` | **BROKEN** | Coherent 9-attack framework with real HTTP transport, but `cargo test` fails to compile (`E0308` in a unit test); several budget/concurrency fields are dead. |
| 8 | `aisec-models` | **PARTIAL** | Real HTTP download, SHA256 verify, and `llama-server` subprocess runtime; tests pass, but inference/download are never exercised live and vault import is incomplete. |
| 9 | `aisec-judge` | **BROKEN** | Real rule/regex/LLM consensus engine, but a shipped integration test fails on a genuine consensus bug (spaced `API key:` slips below threshold). |
| 10 | `aisec-report` | **PARTIAL** | HTML/JSON/SARIF/PDF formatters all real and tested (PDF via `printpdf`); PDF is cruder than documented (single page, no charts). |
| 11 | `aisec-plugin-host` | **PARTIAL** | Real manifest/discovery/lifecycle/subprocess invocation, but 1 unit test fails, sample manifests break discovery, and sandbox/permissions are audit-only. |
| 12 | `desktop-ui` | **PARTIAL** | Polished 9-page React shell, but **entirely mock-fed** with dead action buttons; the Tauri backend is a **SKELETON** (2 IPC commands, no DB/engines). |

**Overall:** AISec is a **strong library prototype with no integration spine**. The domain crates
range from complete to broken-on-test, but **none are wired into the desktop app**. The product
the UI appears to offer does not exist behind the IPC boundary.

---

## Build & test status (verified this session)

| Command | Result |
|---------|--------|
| `cargo build --workspace` | ✅ Pass (warnings only) |
| `npm run build` (tsc + vite) | ✅ Pass |
| `npm test` (vitest) | ✅ Pass (3 tests) |
| `cargo test --workspace` | ❌ Fail (compile errors + hang + failing tests) |

Per-crate `cargo test` results (run individually):

| Crate | `cargo test -p <crate>` | Detail |
|-------|--------------------------|--------|
| `aisec-core` | ✅ 2 passed | — |
| `aisec-storage` | ❌ compile error | `E0599: no method named create` (×2) |
| `aisec-discovery` | ❌ hang | `crawler_respects_max_depth` never returns (worker deadlock) |
| `aisec-payload` | ✅ 10 + 5 passed | — |
| `aisec-models` | ✅ 12 + 5 passed | platform-specific HW detection uses `/proc/meminfo` on Linux |
| `aisec-judge` | ❌ 1 failed | integration: `regex_and_rules_agree_on_secret` (2 passed, 1 failed) |
| `aisec-report` | ✅ 9 + 5 passed | — |
| `aisec-fingerprint` | ✅ 15 passed | — |
| `aisec-plugin-host` | ❌ 1 failed | `permissions::tests::path_glob` (5 passed, 1 failed) |
| `aisec-auth` | ❌ compile error | `E0432/E0433: tokio::process` not found |
| `tests/integration` | ❌ compile error | uses `tracing` without declaring the dependency |

---

## Per-module detail

---

### 1. `aisec-core` — COMPLETE

Shared foundation: `error.rs` (`AisecError`, `ErrorCode`, `AisecResult`) and `logging.rs`
(`tracing` bootstrap with optional daily file appender). 3 source files, 227 LOC.

- **Production code:** `src/error.rs`, `src/logging.rs`, `src/lib.rs` — all real, no I/O beyond optional log-dir creation.
- **Mock code:** None (production or test).
- **Placeholder code:** `src/logging.rs` — `let _ = options.json_file;` (the `json_file` flag is accepted but ignored; file logs are always plain text). `lib.rs` doc claims "domain primitives" that do not exist.
- **TODOs:** None.
- **Missing implementation:** No shared domain types (each crate redefines `Severity`, `Category`, etc.); `ErrorCode::Unauthorized`/`Plugin` only reachable via `Tagged`.
- **Blocking issues:** None. Both unit tests pass.

---

### 2. `aisec-storage` — BROKEN

Real SQLite persistence via `sqlx`: connection pool with WAL + FK pragmas, embedded migration
runner, 12 repository implementations, FTS5 full-text search on findings, auth tables. 20 source
files, ~2,649 LOC + 2 SQL migrations.

- **Production code:** `src/pool.rs` (`sqlx::migrate!("./migrations")` + `MIGRATOR.run` on every `connect()`); `src/repositories/sqlite/*.rs` (real INSERT/SELECT/UPDATE/DELETE); `finding.rs` FTS5 `search()`; `util.rs` UUID v7 + RFC 3339 timestamps. No in-memory/HashMap backends in non-test code.
- **Mock code:** Test-only `test_utils::test_database()` (`sqlite::memory:`, `#[cfg(test)]`).
- **Placeholder code:** None (`todo!`/`unimplemented!`-free). Intentional column defaults only (e.g. `scan.rs` status `"pending"`).
- **TODOs:** None.
- **Missing implementation:** **No encryption at rest** — `cookies_json`, `tokens_json`, `config_json`, file paths stored as plain TEXT (no `aisec-vault`). `docs/DATABASE.md` still says "Version: 001" and omits the auth repository API (migration 002 exists and runs).
- **Blocking issues:** **`cargo test -p aisec-storage` fails to compile (`E0599`).** Root cause: `finding.rs` and `attack_result.rs` test modules call `repos.scans().create(...)` but import only `ProjectRepository`, not the `ScanRepository` trait that defines `create`. The production library itself (`cargo build`) compiles fine — the failure is in the test harness.

---

### 3. `aisec-discovery` — BROKEN

Attack-surface discovery: reqwest HTTP client (redirect cap, body limit, retries), SSRF URL
policy, HTML/JS link extraction, and OpenAPI/GraphQL/REST/AI detectors with static path probes,
driven by a concurrent BFS crawler. 15 source files, ~1,804 LOC. (See also
`docs/DISCOVERY_VERIFICATION_REPORT.md`.)

- **Production code:** `engine.rs`, `client.rs`, `retry.rs`, `url_policy.rs`, `extract.rs`, all `detectors/*.rs`, and `crawler.rs` (single-worker path is reliable). `examples/verify_target.rs` works with `worker_count: 1`.
- **Mock code:** None in production; tests use WireMock (external local server).
- **Placeholder code:** None (`todo!`/`unimplemented!`-free); early returns in `crawler.rs` are real guard logic.
- **TODOs:** `examples/verify_target.rs:15` — `// TODO: use 4+ after crawler deadlock fix (see DISCOVERY_VERIFICATION_REPORT.md)`.
- **Missing implementation (vs `docs/DISCOVERY.md`):** No storage persistence, no `robots.txt`/sitemap, no rate limiting, no authenticated crawl, no DNS-rebinding/redirect SSRF hardening (hostname/literal-IP check only), no OpenAPI path expansion (detects spec URL only), no Tauri wiring. `ProbeOutput.errors` is always empty.
- **Blocking issues:** **Multi-worker crawler deadlock.** In `crawler.rs` the worker pop/`in_flight++` is racy and `notify_one()` wakes only one waiter, so with `worker_count > 1` workers exit/starve and `wait_for_completion()` blocks forever. The default config ships `worker_count: 8`. **`crawler::tests::crawler_respects_max_depth` (worker_count: 2) hangs**, so `cargo test -p aisec-discovery` never completes. The AI probe also skips POST-only endpoints when GET returns 404.

---

### 4. `aisec-auth` — BROKEN

Authentication engine: Playwright subprocess protocol (Node `runner.mjs` driving real Chromium),
JSON-lines RPC client, session store (SQLite + filesystem vault), cookie/token utilities, and a
mock driver for tests. 11 source files, ~1,522 LOC + `playwright/runner.mjs`.

- **Production code:** `engine.rs` (record/replay/authenticate/extract), `playwright/client.rs` + `playwright/runner.mjs` (real Chromium), `session/store.rs`, `cookies.rs`, `playwright/protocol.rs`. The **default** path (`driver: None`) builds a real `PlaywrightClient`, not the mock (`engine.rs:31-34`).
- **Mock code:** `src/mock.rs` `MockPlaywrightDriver` — test-only; injected explicitly in unit/integration tests.
- **Placeholder code:** `authenticate_api_key` discards `header_name` (`let _ = header_name`); `CookieManager::sync_from_browser` exists but is not exposed on `AuthEngine`. No `todo!`/`unimplemented!`.
- **TODOs:** None.
- **Missing implementation (vs `docs/AUTH.md`):** **JWT signature/expiry not validated** — `validate_jwt_structure` only calls `decode_header` and checks segment count (a token ending in `.sig` passes). **No encryption at rest** — vault is plaintext JSON; SQLite cookies/tokens are plain JSON. No OS keychain; no provider-specific OAuth/OIDC/SAML flows; no full profile CRUD on the public API.
- **Blocking issues:** **`cargo test -p aisec-auth` fails to compile (`E0432`/`E0433`).** The crate uses `tokio::process` (`playwright/client.rs:9-10`) plus `tokio::fs`/`AsyncBufReadExt`/`AsyncWriteExt`, but inherits the workspace `tokio` which enables only `["macros","rt-multi-thread","sync","time"]` — `process`/`fs`/`io-util` are not enabled.

---

### 5. `aisec-fingerprint` — COMPLETE

AI provider fingerprinting: declarative rule catalog (59 rules across 8 providers), a real matcher
engine (host/path/header/JSON/status), and a documented confidence-scoring model with
cross-provider conflict penalties. 15 source files, ~1,020 LOC.

- **Production code:** `rules/providers/*.rs` (OpenAI 9, Anthropic 7, Gemini 7, Bedrock 8, Azure 7, Ollama 8, LiteLLM 6, vLLM 7), `evaluator.rs` (regex/header/JSON-pointer matching), `scoring.rs` (`1 - e^(-raw)` + diversity bonus + 0.72 strong-signal floor + penalties), `engine.rs` (+ `fingerprint_batch`).
- **Mock code:** None.
- **Placeholder code:** `scoring.rs` `suggest_method()` hardcodes `Some("POST")` for every provider; `types.rs` `FingerprintInput.method` is stored but never read. No `todo!`/`unimplemented!`.
- **TODOs:** None.
- **Missing implementation:** No coupling to `aisec-discovery` snapshots (usage guidance only); `aisec-core` is a declared-but-unused dependency; `fingerprint_batch()` is undocumented and untested.
- **Blocking issues:** None. All 15 tests pass (one detection test per provider + threshold/scoring tests).

---

### 6. `aisec-payload` — COMPLETE

Payload library + mutation/generation pipeline: 24 embedded adversarial payloads across 10
categories (compile-time `include_str!`), real encoders, and a generation pipeline with
id/category/tag filtering. 7 source files, ~815 LOC + `data/payloads.json`.

- **Production code:** `library/mod.rs` (loads 24 payloads, query API), `mutation/encodings.rs` (Cyrillic homoglyph + zero-width, base64, hex, HTML-entity encoders), `mutation/mod.rs` (apply/apply_chain/expand), `pipeline.rs` (variant generation with UUID lineage + stats).
- **Mock code:** None.
- **Placeholder code:** `Default` impls for `PayloadPipeline`/`PayloadDatabase` panic on catalog-parse failure (acceptable for `Default`). No `todo!`/`unimplemented!`.
- **TODOs:** None.
- **Missing implementation (vs `docs/PAYLOAD.md`):** The **`storage` cargo feature is declared but has zero `#[cfg(feature = "storage")]` code**. `pipeline.expand()` produces only original + single-mutation variants (no combinatorial chains; `apply_chain` is never called by the pipeline); `MutationConfig.max_per_payload` (default 4) silently caps mutation breadth.
- **Blocking issues:** None. All 15 tests pass (10 unit + 5 integration).

---

### 7. `aisec-attack` — BROKEN

AI attack framework: 9 attack categories with real payload sets + heuristic evaluators, a registry,
lifecycle state machine, executor, sequential multi-category orchestrator, and a real HTTP
transport. 27 source files, ~2,624 LOC.

- **Production code:** `transport/http.rs` (reqwest), `payload/mutator.rs` (7 mutators via `aisec-payload`), `payload/runner.rs`, `executor.rs`, `orchestrator.rs`, `registry.rs`, `lifecycle.rs`, `collector.rs`, all 9 `attacks/*.rs` (prompt_injection, system_prompt_extraction, jailbreak, rag_leakage, memory_poisoning, cross_user_leakage, agent_goal_hijacking, tool_abuse, mcp_abuse), `attacks/common.rs`. `default_executor()` uses real `HttpTransport`.
- **Mock code:** `transport/mock.rs` `MockTransport` — public-exported but used only in `#[cfg(test)]`/integration tests; the production path never uses it. (`mock.rs` always returns response index 0.)
- **Placeholder code:** Dead-but-present: `error.rs` `BudgetExhausted`/`Cancelled` never constructed; `orchestrator.rs` `concurrency` field + `let _ = idx` unused; `types.rs` `max_mutations_per_payload`, `TargetKind`, and `PayloadFormat::MultiTurn` defined but never used; `traits.rs` `supported_mutators()` never called. No `todo!`/`unimplemented!`.
- **TODOs:** None.
- **Missing implementation (vs `docs/ATTACK.md`):** `OrchestratorConfig.concurrency` is **dead** (strictly sequential `for` loop); `AttackBudget.max_mutations_per_payload` is **not enforced** (mutator uses hardcoded `MutatorConfig.max_per_payload: 3`); no result persistence (`ResultSink` trait only; `storage` feature unused); no per-`TargetKind` routing (everything uses the OpenAI chat template); no multi-turn attacks; no cancellation.
- **Blocking issues:** **`cargo test -p aisec-attack` fails to compile (`E0308`).** `payload/runner.rs` test calls `PayloadRunner::new(transport)` with an owned value, but the constructor requires `&'a T` (production `executor.rs:77` correctly passes `&self.transport`). Compilation fails before any test runs.

---

### 8. `aisec-models` — PARTIAL

Local GGUF model manager: in-memory registry, resumable HuggingFace download, streaming SHA256
verification, platform-specific hardware detection, and a **real** `llama-server` subprocess
inference runtime. 14 source files, ~1,696 LOC.

- **Production code:** `download/huggingface.rs` + `download/manager.rs` (real Range-resumable HTTP), `verify.rs` (streaming SHA256), `registry.rs`, `manager.rs` (real orchestration; `download_huggingface()` hits real HF URLs), `runtime/llama_cpp.rs` (spawns `llama-server`, polls `/health`, POSTs `/completion`), `hardware/detect.rs` (macOS `sysctl`/`system_profiler`; Linux `/proc/meminfo`/`nvidia-smi`).
- **Mock code:** `runtime/mock.rs` `MockInferenceRuntime` — test-only; `LocalModelManager` always embeds the real `LlamaCppRuntime`, never the mock.
- **Placeholder code:** Unused enum variants (`DownloadStatus::Paused`, `GpuBackend::Vulkan`/`Rocm`); non-Linux/macOS RAM fallback hardcodes 8 GiB; `recommended_gpu_layers()` returns flat `35`/`0` (no VRAM tuning); `lib.rs` doc says `~/.aisec/models` but code uses `AISEC_MODEL_VAULT` or `./data/models`.
- **TODOs:** None.
- **Missing implementation (vs `docs/MODELS.md`):** `import_local()` keeps the original path instead of copying into the vault layout; `storage` feature unused; no end-to-end test against real HuggingFace or `llama-server`.
- **Blocking issues:** None on this Linux toolchain — 12 unit + 5 integration tests pass (tests mock network via WireMock and inference via `MockInferenceRuntime`). **Runtime caveat:** real inference requires an external `llama-server` binary on `PATH`.

---

### 9. `aisec-judge` — BROKEN

AI judge engine: rule-based + regex-based + LLM evaluators with weighted multi-model consensus and
severity scoring. 13 source files, ~1,413 LOC.

- **Production code:** `engine.rs` (rule → regex → LLM pipeline), `evaluators/rule.rs` (category substring + refusal rules), `evaluators/regex.rs` (5 compiled `(?i)` patterns), `evaluators/llm.rs` (real `InferenceRuntime::complete()` — production = `LlamaCppRuntime`), `scoring.rs` (weighted consensus), `consensus.rs`, `roles.rs`, `prompts.rs`.
- **Mock code:** `mock_runtime.rs` `JsonMockRuntime` — test-only; used in all integration tests and `engine_with_mock()`.
- **Placeholder code:** `mock_runtime.rs` has an empty `if !self.ready {}` block (dead). No `todo!`/`unimplemented!`.
- **TODOs:** None.
- **Missing implementation (vs `docs/JUDGE.md`):** Not referenced anywhere else in the workspace (not wired into `aisec-attack`). Only 3 category rule sets exist (everything else falls back to `prompt_injection`); no RAG/tool/MCP-specific rules. LLM evaluator errors are silently skipped (debug log only).
- **Blocking issues:** **Integration test `regex_and_rules_agree_on_secret` fails** — a genuine product bug. Input `"The API key: sk-live-abc123"`: the credential regex `(api[_-]?key|...)\s*[:=]` does not match the space in `"API key:"`, so only the rule evaluator (weight 0.35, conf 0.9) votes vulnerable. Consensus = `0.315 / 0.8 = 0.394 < 0.55` threshold → `vulnerable: false`. So the deterministic (no-LLM) path under-reports an obvious secret leak.

---

### 10. `aisec-report` — PARTIAL

Reporting engine: HTML, JSON, SARIF, and PDF formatters with chart rendering, recommendations, and
compliance references. 12 source files, ~1,854 LOC.

- **Production code:** `engine.rs` (formatter selection + file writes), `formatters/html.rs` (dark theme + embedded SVG charts), `formatters/json.rs` (serde), `formatters/sarif.rs` (SARIF 2.1.0 + severity mapping), `formatters/pdf.rs` (**real `printpdf` 0.7** — output begins with `%PDF`), `charts.rs` (SVG + ASCII), `recommendations.rs`, `data.rs`.
- **Mock code:** None — no stub formatters.
- **Placeholder code:** None (`todo!`/`unimplemented!`-free).
- **TODOs:** None.
- **Missing implementation (vs `docs/REPORT.md`):** PDF is **single physical page** — on overflow it resets the Y coordinate on the same page (overlay risk) instead of paginating; PDF omits the documented risk-gauge and category charts (only a text severity chart); PDF text is ASCII-sanitized; SARIF `message.text` carries the title only (no description/evidence in the result body); NIST mapping is string refs, not structured controls.
- **Blocking issues:** None. All 14 tests pass (9 unit + 5 integration).

---

### 11. `aisec-plugin-host` — PARTIAL

Plugin host: `aisec-plugin.toml` manifest parsing/validation, recursive discovery, lifecycle state
machine, a subprocess sandbox runner (JSON-lines protocol), and a permission guard. 9 source files,
~1,120 LOC. Plus plugin SDKs (`packages/plugin-sdk-{python,js}`) and 4 sample plugins.

- **Production code:** `manifest.rs` (TOML parse + SemVer/api-version/subprocess validation), `manager.rs` (recursive discovery, install/enable/disable, async invoke), `lifecycle.rs` (validated transitions), `sandbox/runner.rs` (spawn `python3`/`node`, JSON-lines invoke + shutdown, 30s timeout), `permissions.rs` (method→capability mapping).
- **Mock code:** None in production (a `print('stub')` temp plugin appears only in a `#[cfg(test)]` test).
- **Placeholder code:** `types.rs` `SandboxConfig.max_output_bytes` defined but never enforced; `permissions.rs` `check_path_read()` implemented but never called from the runner; env "stripping" hardcoded to 2 keys (`AWS_SECRET_ACCESS_KEY`, `OPENAI_API_KEY`); `$PLUGIN_DIR` allowlist token never expanded; host API calls are logged only (no host-side execution/enforcement). Unused deps (`async-trait`, `time`) and import (`warn`).
- **TODOs:** None.
- **Missing implementation (vs `docs/PLUGINS.md`):** No OS-level isolation (no seccomp/cgroups/namespaces/WASM — subprocess only, full interpreter privileges); `AISEC_NO_NETWORK=1` is a convention, not enforced; permission enforcement is **audit-only** (records `allowed: false` but the plugin keeps running); host APIs (`http_request`, `filesystem_*`, `read_resource`) have no implementation; "Loaded" spawns a new subprocess per invoke (not persistent); no storage/orchestrator/Tauri wiring.
- **Blocking issues:** **`permissions::tests::path_glob` fails** (5/6 pass) because `$PLUGIN_DIR/**` is matched literally (no substitution). **Sample manifest schema mismatch:** the parser's `PermissionsRationale` uses `#[serde(flatten)] HashMap<String,String>` (flat keys under `[permissions]`), but `discovery-openapi-paths`, `attack-delimiter-injection`, and `_template` manifests use a nested `[permissions.rationale]` table → serde error → **discovery of those samples fails** (breaks `discovers_all_sample_plugins`). Sample *code* matches the protocol; the 2 affected *manifests* do not.
- **SDK/sample note:** SDKs are usable standalone (JSON-lines runtime) but lack documented `PluginContext` helpers (`http_request`, filesystem, `probe_mutate`); typed bases are abstract `NotImplementedError`/`throw` stubs by design.

---

### 12. `desktop-ui` — PARTIAL  *(frontend PARTIAL · backend SKELETON · data MOCK)*

React 19 + TypeScript + Vite frontend (~2,670 LOC) in a Tauri 2 shell (~154 LOC Rust).

- **Production code (frontend):** Real app shell — `main.tsx`, `App.tsx` (boot + IPC health probe), `AppRouter` (HashRouter, lazy routes), `MainLayout`/`Sidebar`/`TopBar`, a shared component library (Button, Card, Badge, DataTable, etc.), Context+`useReducer` store, and working client-side filtering (search, severity chips, project selection). 9 feature pages render (Dashboard, Projects, Targets, Discovery, Attacks, Findings, Reports, Models, Settings).
- **Production code (backend):** `src-tauri/src/{main,lib,state,error,logging}.rs` + `commands/mod.rs` — real Tauri bootstrap with `aisec-core` logging and a `CommandError` IPC envelope.
- **Mock code:** **All domain data is mock.** `src/shared/mock/data.ts` seeds the store (3 projects, 6 targets, 7 findings, etc.). Even when Tauri reports "Connected", the data stays mock — the connection flag never switches the data source (`App.tsx` only sets `backendConnected`).
- **Placeholder code:** ~30 dead `<Button>`s with no handlers (New Project, Add Target, Start Scan, Launch Attack, Generate Report, Download Model, etc.). No `<form>` submits, no modals, no detail views. `EmptyState` component is exported but unused. No TODO/FIXME comments.
- **IPC reality:** Frontend calls only `health` and `app_info`; the Tauri shell exposes only `health` and `app_info` (`lib.rs:22`). No domain commands, no events/streams.
- **Backend reality:** `src-tauri/Cargo.toml` depends only on `aisec-core` — **no `aisec-storage`, no engines**. `AppState` holds only a log guard; **no database is opened**, no engine is invoked.
- **State management:** React Context + `useReducer` (not Zustand). All mutations (finding status, settings, theme) are in-memory/session-only; nothing persists.
- **Missing implementation:** Every product action (create/import/scan/attack/report/model management) is unwired; no persistence; no real data hydration; no run console / test designer / plugin manager.
- **Blocking issues:** None for its current scope — `npm run build` and `npm test` pass. **Product risk:** the "Connected" badge can imply live data while everything shown is mock.

---

## Cross-cutting findings

### Integration spine is absent
The single most important finding: **the Tauri app links only `aisec-core`.** None of the 10 domain
crates (storage, discovery, fingerprint, payload, attack, models, judge, report, auth, plugin-host)
are referenced by the desktop binary or by each other beyond shared types. There is no orchestrator,
no `discovery → fingerprint → attack → judge → storage → report` pipeline, and no IPC surface beyond
`health`/`app_info`. The crates are an **SDK**, not a wired product.

### Security controls are on paper
- **No encryption at rest:** `aisec-storage` and `aisec-auth` persist credentials/cookies/tokens as plain JSON; no `aisec-vault`.
- **JWT is structure-only:** no signature/expiry verification (`aisec-auth`).
- **Plugin sandbox is a bare subprocess:** no seccomp/cgroups/WASM; permission guard is audit-only; env stripping covers 2 keys.
- **SSRF guard is shallow:** hostname/literal-IP only; no DNS-rebinding or redirect re-validation (`aisec-discovery`).

### Test suite cannot go green as-is
`cargo test --workspace` cannot pass without code changes: **3 compile failures** (`aisec-storage`,
`aisec-auth`, `tests/integration`), **1 hang** (`aisec-discovery`), and **2 failing tests**
(`aisec-judge`, `aisec-plugin-host`). One additional compile failure (`aisec-attack`) blocks its own
test target. No CI workflow exists to catch regressions.

### Dead configuration & doc drift
Several documented features are dead fields: `OrchestratorConfig.concurrency`,
`AttackBudget.max_mutations_per_payload`, `SandboxConfig.max_output_bytes`, the unimplemented
`storage` cargo feature across multiple crates. `docs/DATABASE.md` (v001) and `docs/PROJECT_STRUCTURE.md`
are stale relative to the current tree.

---

## Module classification recap

| COMPLETE | PARTIAL | SKELETON | MOCK | BROKEN |
|----------|---------|----------|------|--------|
| `aisec-core` | `aisec-models` | *(Tauri backend, within `desktop-ui`)* | *(UI data layer, within `desktop-ui`)* | `aisec-storage` |
| `aisec-fingerprint` | `aisec-report` | | | `aisec-discovery` |
| `aisec-payload` | `aisec-plugin-host` | | | `aisec-auth` |
| | `desktop-ui` | | | `aisec-attack` |
| | | | | `aisec-judge` |

*`desktop-ui` is classified PARTIAL overall; its data layer is MOCK and its Tauri backend is a
SKELETON.*

---

*Point-in-time audit. Re-run `cargo build --workspace`, `cargo test --workspace`, `npm run build`,
and `npm test` after any remediation to refresh status.*
