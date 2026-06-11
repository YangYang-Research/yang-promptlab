# AISec Module Status

**Last updated:** 2026-06-10  
**Build baseline:** `cargo build --workspace` pass · `npm run build` pass  
**Reference:** `docs/ARCHITECTURE.md`, `docs/AUDIT_REPORT.md`

---

## Classification Key

| Status | Definition |
|--------|------------|
| **COMPLETE** | Intended scope implemented; builds; core tests pass; wired into product flow |
| **PARTIAL** | Substantial implementation; missing integration, features, or has non-blocking defects |
| **SKELETON** | Structure and stubs only; minimal or placeholder behavior |
| **BROKEN** | Does not compile, primary tests fail, or end-to-end path is non-functional |

Classifications reflect **product readiness** (desktop app + backend integration), not library-only completeness.

---

## Summary

| Module | Status | Primary location |
|--------|--------|------------------|
| UI | **PARTIAL** | `src/` |
| Database | **PARTIAL** | `crates/aisec-storage` |
| Discovery | **PARTIAL** | `crates/aisec-discovery` |
| Authentication | **PARTIAL** | `crates/aisec-auth` |
| Attack Framework | **PARTIAL** | `crates/aisec-attack`, `crates/aisec-payload` |
| Judge Engine | **PARTIAL** | `crates/aisec-judge` |
| Reporting | **PARTIAL** | `crates/aisec-report` |
| Plugin SDK | **BROKEN** | `crates/aisec-plugin-host`, `packages/plugin-sdk-*`, `plugins/` |
| Model Manager | **PARTIAL** | `crates/aisec-models` |

**Overall:** 0 COMPLETE · 8 PARTIAL · 0 SKELETON · 1 BROKEN

No module is product-complete. Domain libraries are largely built; application integration (Tauri IPC, persistence hydration, run lifecycle) is absent across the board.

---

## Module Details

### UI — PARTIAL

**Location:** `src/` (React 19 + TypeScript + Vite)

| Aspect | State |
|--------|-------|
| Pages | 9 feature pages: Dashboard, Projects, Targets, Discovery, Attacks, Findings, Reports, Models, Settings |
| Shell | Layout, sidebar, routing (`HashRouter`), shared components, dark theme |
| State | React Context + `useReducer` (`src/app/store/`) — not Zustand per architecture |
| Data | 100% mock (`src/shared/mock/data.ts`); no CRUD via backend |
| IPC | `healthCheck`, `getAppInfo` only (`src/shared/ipc/client.ts`) |
| Build | `npm run build` pass |
| Tests | 3 vitest tests (logger, errors only) |

**Done**
- Full navigation shell and page scaffolding
- Backend health probe on bootstrap; graceful fallback to mock mode
- Finding status updates in local state

**Gaps**
- No domain IPC (projects, scans, findings, reports, models, plugins)
- No Run Console, Test Designer, or Plugin Manager pages
- No event subscriptions / streaming for live runs
- Dashboard metrics derived from mocks (attack runs undercounted)

**Blockers to COMPLETE**
- Tauri command surface for all feature domains
- Replace mock store with IPC-backed hydration
- Run progress and finding ingestion from backend events

---

### Database — PARTIAL

**Location:** `crates/aisec-storage`

| Aspect | State |
|--------|-------|
| Engine | SQLite via sqlx |
| Migrations | `001_initial_schema.sql`, `002_auth_schema.sql` |
| Repositories | Projects, targets, scans, findings (FTS5), attack results, reports, models, plugins, payloads, auth profiles/sessions/recordings |
| Build | `cargo build -p aisec-storage` pass |
| Tests | Lib tests **do not compile** — `ScanRepository` trait not in scope in `finding.rs` and `attack_result.rs` test modules |
| Integration | **Not opened by Tauri** — `src-tauri` depends on `aisec-core` only |

**Done**
- Full relational schema with indexes and FTS for findings
- Async repository traits + SQLite implementations
- In-memory test database helper

**Gaps**
- No encryption at rest (`aisec-vault` absent; secrets stored as JSON)
- No connection from desktop app to `Database` pool
- Test suite broken (compile errors)

**Blockers to COMPLETE**
- Wire `Database` into Tauri `AppState`
- Fix trait imports in repository tests
- IPC commands for repository operations

---

### Discovery — PARTIAL

**Location:** `crates/aisec-discovery`

| Aspect | State |
|--------|-------|
| Engine | `DiscoveryEngine` — crawl, extract, enumerate attack surface |
| Detectors | OpenAPI, GraphQL, REST/API paths, AI/LLM routes, static path probes |
| Policy | URL validation, same-origin, private-network block (hostname literal) |
| Build | Pass |
| Tests | Integration tests (wiremock); slow in audit environment (>2 min) |

**Done**
- Configurable crawler (depth, page limits, workers, timeout)
- HTTP client with retry
- Structured `DiscoveryReport` output

**Gaps**
- Not invoked from UI or Tauri
- SSRF policy does not resolve DNS or re-validate redirects
- No persistence of discovery results to `aisec-storage` from app flow

**Blockers to COMPLETE**
- IPC `discovery.start` / progress events
- Persist endpoints to targets/scans tables
- Harden URL policy for pentest safety

---

### Authentication — PARTIAL

**Location:** `crates/aisec-auth`, auth tables in `crates/aisec-storage`

| Aspect | State |
|--------|-------|
| Engine | `AuthEngine` — profile CRUD, session lifecycle |
| Playwright | Subprocess protocol (`playwright/runner.mjs`), client driver |
| Sessions | Cookie/token extraction, session store, storage-state paths |
| JWT | Structural decode — no signature/expiry enforcement |
| Build | Pass |
| Tests | Lib tests **do not compile** — `tokio::process` feature not enabled on workspace `tokio` |

**Done**
- Auth profile, session, recording repository traits in storage
- Mock Playwright driver for unit testing (when compile fixed)
- Configurable auth engine

**Gaps**
- Not wired to UI or scan runs
- Credentials/tokens not encrypted
- Playwright runtime not bundled in `resources/`
- Real browser automation path untested in CI

**Blockers to COMPLETE**
- Enable `tokio` `process` feature; fix lib tests
- IPC for auth profile management and session replay
- Integrate authenticated transport into attack/discovery flows

---

### Attack Framework — PARTIAL

**Location:** `crates/aisec-attack`, `crates/aisec-payload`

| Aspect | State |
|--------|-------|
| Categories | 9 built-in: prompt injection, jailbreak, tool abuse, MCP abuse, RAG leakage, agent goal hijacking, memory poisoning, and related |
| Core | Registry, executor, lifecycle, HTTP/mock transport, payload runner/mutator |
| Orchestrator | `AttackOrchestrator` — sequential category execution |
| Build | Pass |
| Tests | Integration 2/2 pass; lib test **does not compile** (`PayloadRunner::new` missing borrow) |

**Done**
- Trait-based attack plugins with structured results
- Payload library, mutation pipeline (`aisec-payload` — integration 5/5 pass)
- Result collector and orchestration report types

**Gaps**
- `OrchestratorConfig.concurrency` unused (always sequential)
- `AttackBudget.max_mutations_per_payload` not enforced
- Not connected to judge, storage, or UI
- No scan/run lifecycle in Tauri

**Blockers to COMPLETE**
- IPC run orchestration (start, abort, progress)
- Wire executor → judge → storage pipeline
- Fix lib test compile error; honor budget/concurrency config

---

### Judge Engine — PARTIAL

**Location:** `crates/aisec-judge`

| Aspect | State |
|--------|-------|
| Evaluators | Rule-based, regex, LLM (via `aisec-models` runtime) |
| Consensus | Multi-model weighted voting, confidence scoring |
| Build | Pass |
| Tests | Integration **2/3 pass** — `regex_and_rules_agree_on_secret` fails (regex pattern vs `"API key:"` mismatch; consensus threshold 0.55) |

**Done**
- Deterministic and LLM-augmented judgment paths
- Role pool (judge, classifier, attacker)
- Engine unit tests pass for direct leak cases (`password: secret123`)

**Gaps**
- False negative on spaced credential patterns (`"The API key: …"`)
- LLM evaluator errors silently skipped
- Not invoked post-attack in product flow
- No IPC exposure

**Blockers to COMPLETE**
- Fix regex/consensus alignment and failing integration test
- Wire into attack pipeline automatically
- IPC for manual re-judge / triage

---

### Reporting — PARTIAL

**Location:** `crates/aisec-report`

| Aspect | State |
|--------|-------|
| Formats | HTML, PDF, JSON, SARIF 2.1 |
| Features | Charts, recommendations, compliance refs, severity aggregation |
| Build | Pass |
| Tests | Integration **5/5 pass** |

**Done**
- `ReportingEngine` with pluggable formatters
- `ReportDataBuilder` for assembling report payloads
- Strongest test coverage among domain modules

**Gaps**
- UI export buttons operate on mock data only
- No read-from-storage → generate → save flow in Tauri
- No redaction engine or artifact vault attachment

**Blockers to COMPLETE**
- IPC `report.export(format, scan_id)`
- Load findings/run logs from `aisec-storage`
- UI download/save integration

---

### Plugin SDK — BROKEN

**Location:** `crates/aisec-plugin-host`, `packages/plugin-sdk-python`, `packages/plugin-sdk-js`, `plugins/samples/`

| Aspect | State |
|--------|-------|
| Host | `PluginManager`, lifecycle, subprocess `SandboxRunner`, `PermissionGuard` |
| SDKs | Python + JavaScript hook bases, JSON-lines protocol |
| Samples | 4 reference plugins (discovery, attack, judge, report) |
| Build | Pass |
| Tests | Sample plugin tests **0/5 pass** — manifest parse error: `[permissions.rationale]` table invalid (parser expects string) |

**Done**
- Plugin discovery, manifest schema (host-side)
- Permission model and hook invocation protocol
- SDK packages with README

**Gaps**
- Sample manifests incompatible with host parser — plugins cannot load
- Subprocess sandbox only (architecture specifies WASM)
- Permissions recorded but plugins run with full OS privileges
- Not integrated with orchestrator or UI Plugin Manager

**Why BROKEN**
- End-to-end plugin path fails at manifest load; all sample plugin tests panic
- Product cannot install or invoke any bundled sample without schema fix

**Blockers to COMPLETE**
- Align manifest schema (samples ↔ `PluginManifest` parser)
- Fix and pass `sample_plugins` integration tests
- IPC plugin list/enable/invoke; UI Plugin Manager page

---

### Model Manager — PARTIAL

**Location:** `crates/aisec-models`

| Aspect | State |
|--------|-------|
| Manager | `LocalModelManager` — import, registry, verify |
| Download | HuggingFace client with resume |
| Runtime | llama.cpp wrapper + mock runtime |
| Hardware | GPU/RAM detection (platform-specific) |
| Build | Pass |
| Tests | Lib **9/12 pass**, 3 fail on macOS (`hw.memsize` sysctl `S64` parsing); integration tests present |

**Done**
- GGUF verification (SHA256)
- Model registry and vault path configuration
- Inference runtime trait for judge/attack LLM paths

**Gaps**
- Not wired to Settings/Models UI (mock model list)
- llama.cpp binaries not bundled in `resources/llama`
- Hardware detection fragile across platforms
- No IPC for model load/unload/inference status

**Blockers to COMPLETE**
- Fix macOS hardware detection tests
- IPC model catalog, import, and runtime control
- UI hydration from real registry; bundle or document runtime deps

---

## Integration Gap (Cross-Cutting)

All **PARTIAL** modules share the same root cause: the Tauri shell is a bootstrap stub.

```
React UI  ──IPC──►  src-tauri  ──X──►  aisec-storage / engines
              health, app_info only
```

Until domain IPC and `AppState` database wiring exist, no module can reach **COMPLETE**.

---

## Suggested Fix Order

| Priority | Module | Action |
|----------|--------|--------|
| P0 | Plugin SDK | Fix manifest schema drift; restore sample plugin tests |
| P0 | Database | Fix test compile errors; open pool in Tauri |
| P0 | UI | Add IPC commands; hydrate store from storage |
| P1 | Attack Framework | Wire run lifecycle: discovery → attack → judge → storage |
| P1 | Judge Engine | Fix failing integration test |
| P1 | Reporting | Connect export to storage-backed findings |
| P2 | Authentication | Enable tokio process feature; auth IPC |
| P2 | Model Manager | Fix hardware tests; model IPC |
| P2 | Discovery | IPC + SSRF hardening |

---

## Verification Commands

```bash
# Build
cargo build --workspace
npm run build

# Per-module tests
cargo test -p aisec-storage --lib          # compile fail
cargo test -p aisec-discovery --test integration
cargo test -p aisec-auth --lib              # compile fail
cargo test -p aisec-attack --test integration
cargo test -p aisec-judge --test integration  # 1 fail
cargo test -p aisec-report --test integration
cargo test -p aisec-models --lib            # 3 fail (macOS)
cargo test -p aisec-plugin-host --test sample_plugins  # 5 fail
npm test
```

---

*Regenerate this document after integration milestones or test suite fixes.*
