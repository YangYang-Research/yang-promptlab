# Mock Implementation Inventory

**Date:** 2026-06-10  
**Scope:** Full repository audit  
**Search patterns:** Mock, Fake, Placeholder, Stub, TODO, `unimplemented!`, `todo!`, hardcoded response, sample data, mock server

---

## Summary

| Category | Count | MVP impact |
|----------|-------|------------|
| **Product mocks** (user-facing, blocks MVP) | 18 | High — must replace |
| **Bootstrap stubs** (Tauri IPC shell) | 3 | High — must extend |
| **UI placeholders** (inert buttons/controls) | 14 | High — wire to IPC |
| **Test doubles** (intentional) | 12 | None — keep for CI |
| **Dev fixtures / mock servers** | 5 | None — dev/test only |
| **Reference samples / templates** | 6 | Low — not product path |
| **TODO / tracked gaps** | 1 | Medium — crawler fix |

**No `unimplemented!()` or `todo!()` macros** found in source code.

---

## Category A — Product Mocks (Replace for MVP)

These mocks stand in for real backend data or services in the shipped application path.

### A1. Frontend mock data store

| # | File | Function / export | Why it is a mock | Real implementation required |
|---|------|-------------------|------------------|------------------------------|
| 1 | `src/shared/mock/data.ts` | `mockProjects` | Hardcoded 3 fake projects (Acme Chatbot, etc.) | Load from `project_list` IPC → SQLite |
| 2 | `src/shared/mock/data.ts` | `mockTargets` | Hardcoded targets with fake URLs/fingerprints | Load from `target_list` IPC |
| 3 | `src/shared/mock/data.ts` | `mockDiscoveryJobs` | Hardcoded scan jobs with fake progress | Load from scan/discovery status IPC or `scan_run` events |
| 4 | `src/shared/mock/data.ts` | `mockAttackRuns` | Hardcoded attack run history | Load from `attack_results` / scan IPC |
| 5 | `src/shared/mock/data.ts` | `mockFindings` | Hardcoded 6+ findings with fake evidence | Load from `finding_list` IPC |
| 6 | `src/shared/mock/data.ts` | `mockReports` | Hardcoded report metadata | Load from `report_list` / `report_generate` IPC |
| 7 | `src/shared/mock/data.ts` | `mockModels` | Hardcoded GGUF model entries | Load from `promptlab-models` registry via IPC |
| 8 | `src/shared/mock/data.ts` | `mockActivity` | Hardcoded activity feed | Derive from scan/finding events or audit log |
| 9 | `src/shared/mock/data.ts` | `computeDashboardStats` | Computes stats from mock arrays (attack runs excluded from `runningScans`) | Compute from real DB counts |
| 10 | `src/shared/mock/data.ts` | `severityCounts` | Helper over in-memory findings | Same helper OK; feed real findings |

### A2. App state initialization

| # | File | Function | Why it is a mock | Real implementation required |
|---|------|----------|------------------|------------------------------|
| 11 | `src/app/store/AppStore.tsx` | `initialState` | Seeds all domain arrays from `mock*` exports | Hydrate from IPC on boot; empty arrays when DB empty |
| 12 | `src/app/store/AppStore.tsx` | `UPDATE_FINDING_STATUS` reducer | Updates finding status in memory only | `finding_update` IPC → SQLite |
| 13 | `src/App.tsx` | `bootstrap()` catch branch | On IPC failure, sets `backendConnected: false` and keeps mock data | Optional mock fallback OK for dev; primary path loads DB |
| 14 | `src/app/layout/TopBar.tsx` | Status badge | Shows "Mock mode" when backend disconnected | Show real connection + sync state |

### A3. IPC client (backend surface)

| # | File | Function | Why it is a mock | Real implementation required |
|---|------|----------|------------------|------------------------------|
| 15 | `src/shared/ipc/client.ts` | `healthCheck()` | Only verifies Tauri alive; no domain data | Keep; add domain commands alongside |
| 16 | `src/shared/ipc/client.ts` | `getAppInfo()` | Returns static metadata | Keep; add `projectCreate`, `targetCreate`, `scanRun`, etc. |

### A4. Tauri application shell

| # | File | Function | Why it is a mock | Real implementation required |
|---|------|----------|------------------|------------------------------|
| 17 | `src-tauri/src/state.rs` | `AppState::new` | Holds log guard only; no `Database` | `AppState { db: Database, paths }` |
| 18 | `src-tauri/src/commands/mod.rs` | `health()`, `app_info()` | Bootstrap stubs; return static JSON | Extend with CRUD + `scan_run` + `report_generate` |
| 19 | `src-tauri/Cargo.toml` | dependencies | Only `promptlab-core`; no storage/engines | Add `promptlab-storage`, `promptlab-discovery`, `promptlab-attack`, `promptlab-judge`, `promptlab-report` |

---

## Category B — UI Placeholders (Inert Controls)

Buttons and actions rendered in UI but **not connected** to backend. Data comes from mocks (Category A).

| # | File | Element | Why it is a placeholder | Real implementation required |
|---|------|---------|-------------------------|------------------------------|
| 1 | `src/features/projects/ProjectsPage.tsx` | "New Project" button | No `onClick`, no modal | Create project form → `project_create` IPC |
| 2 | `src/features/projects/ProjectsPage.tsx` | "Import" button | No handler | Import project JSON (post-MVP) or remove |
| 3 | `src/features/targets/TargetsPage.tsx` | "Add Target" button | No handler (page header) | Add target modal → `target_create` IPC |
| 4 | `src/features/discovery/DiscoveryPage.tsx` | "Start Scan" button | No handler | Invoke `scan_run` IPC |
| 5 | `src/features/discovery/DiscoveryPage.tsx` | "Run Now" / "Pause" / "View Results" | No handlers per job card | Wire to scan lifecycle IPC |
| 6 | `src/features/attacks/AttacksPage.tsx` | `attackCategories` constant | Hardcoded 9 categories in TS, not from `promptlab-attack` registry | Optional: fetch from backend; MVP uses `scan_run` only |
| 7 | `src/features/attacks/AttacksPage.tsx` | "Launch Attack" button | No handler | Part of `scan_run` or separate `attack_run` |
| 8 | `src/features/attacks/AttacksPage.tsx` | "Configure" per category | No handler | Post-MVP playbook designer |
| 9 | `src/features/findings/FindingsPage.tsx` | Status dropdown actions | Local reducer only | `finding_update` IPC |
| 10 | `src/features/reports/ReportsPage.tsx` | "Generate Report" button | No handler | `report_generate` IPC |
| 11 | `src/features/reports/ReportsPage.tsx` | Format cards (PDF/HTML/JSON/SARIF) | Display only | Wire HTML card for MVP |
| 12 | `src/features/models/ModelsPage.tsx` | "Download Model" / "Browse HuggingFace" | No handler | `promptlab-models` IPC (post-MVP) |
| 13 | `src/features/settings/SettingsPage.tsx` | `pluginsDir`, `modelsDir` settings | Local state only; not applied to backend | Persist settings; pass to Tauri |
| 14 | `src/features/dashboard/DashboardPage.tsx` | Dashboard metrics | Derived from mock store via `computeDashboardStats` | Same UI; real data source |

---

## Category C — Test Doubles (Intentional — Keep)

Rust/TS test infrastructure. **Not product mocks** — document for completeness.

### C1. Attack transport mock

| File | Type / function | Why it is a mock | Real implementation |
|------|-----------------|------------------|---------------------|
| `crates/promptlab-attack/src/transport/mock.rs` | `MockTransport`, `MockTransport::ok()`, `send()` | Returns canned HTTP responses; records requests for assertions | **`HttpTransport`** (`transport/http.rs`) — already implemented for production |

### C2. Auth Playwright mock

| File | Type / function | Why it is a mock | Real implementation |
|------|-----------------|------------------|---------------------|
| `crates/promptlab-auth/src/mock.rs` | `MockPlaywrightDriver`, `login_success()` | Returns fixed cookies/tokens without Node/Playwright | **`PlaywrightClient`** (`playwright/client.rs`) + `runner.mjs` subprocess |

### C3. Inference runtime mocks

| File | Type / function | Why it is a mock | Real implementation |
|------|-----------------|------------------|---------------------|
| `crates/promptlab-models/src/runtime/mock.rs` | `MockInferenceRuntime`, `complete()` | Returns `"[mock: prompt]"` text | **`LlamaCppRuntime`** (`runtime/llama_cpp.rs`) |
| `crates/promptlab-judge/src/mock_runtime.rs` | `JsonMockRuntime`, `judge_vulnerable()`, `classifier()` | Returns fixed JSON verdict strings for judge tests | **`LlamaCppRuntime`** via `ModelRolePool` or **`judge_deterministic()`** for MVP |

### C4. Storage test database

| File | Function | Why it is a mock | Real implementation |
|------|----------|------------------|---------------------|
| `crates/promptlab-storage/src/pool.rs` | `test_utils::test_database()` | In-memory SQLite (`sqlite::memory:`) | File-backed `Database::connect("sqlite://~/.promptlab/promptlab.db")` in Tauri |

### C5. Frontend test mocks

| File | Function | Why it is a mock | Real implementation |
|------|----------|------------------|---------------------|
| `tests/frontend/logger.test.ts` | `vi.spyOn(console, "info").mockImplementation` | Vitest console stub | N/A — test only |

### C6. Plugin host unit test stub

| File | Function | Why it is a mock | Real implementation |
|------|----------|------------------|---------------------|
| `crates/promptlab-plugin-host/src/manager.rs` (test) | writes `plugin.py` with `print('stub')` | Minimal fake plugin for discovery test | Real plugins in `plugins/samples/` |

---

## Category D — Mock HTTP Servers (Test & Dev Fixtures)

| # | File | Function / component | Why it is a mock server | Real implementation |
|---|------|----------------------|-------------------------|---------------------|
| 1 | `scripts/discovery-test-target.py` | `Handler.do_GET`, `do_POST` | Python HTTP server on `:3000` with canned routes for verification | Real customer target URLs |
| 2 | `crates/promptlab-discovery/tests/integration.rs` | `MockServer` (wiremock) | In-process HTTP mock for discovery integration test | Live target or test fixture server |
| 3 | `crates/promptlab-models/tests/integration.rs` | `MockServer` | Mocks HuggingFace download URLs | Real HuggingFace CDN |
| 4 | `crates/promptlab-models/src/download/manager.rs` (tests) | `MockServer` | Download/resume unit tests | Real HTTP download |
| 5 | `crates/promptlab-attack/tests/integration.rs` | Uses `MockTransport` (not HTTP server) | In-memory response fake | `HttpTransport` to real API |

---

## Category E — Reference Samples & Templates

Sample plugins and templates for SDK documentation — **not wired to product orchestrator**.

| # | File | Entry point | Why it is sample/stub | Real implementation |
|---|------|-------------|----------------------|---------------------|
| 1 | `plugins/_template/plugin.py` | `discover()` | Returns empty `{"endpoints": [], "count": 0}` | User-authored plugin with real logic |
| 2 | `plugins/samples/discovery-openapi-paths/plugin.py` | `discover()` | Returns hardcoded path list from `COMMON_PATHS` | Plugin host + `DiscoveryEngine` integration |
| 3 | `plugins/samples/attack-delimiter-injection/plugin.js` | `executeAttack()` | Demo mutation only | Plugin invoked from attack pipeline |
| 4 | `plugins/samples/judge-keyword/plugin.py` | `evaluate()` | Keyword stub judge (not `promptlab-judge`) | `promptlab-judge` engine or plugin hook to judge |
| 5 | `plugins/samples/report-markdown-summary/plugin.js` | Report hook | Sample markdown formatter | `promptlab-report` HTML/PDF pipeline |
| 6 | `plugins/samples/*/promptlab-plugin.toml` | manifest | Reference manifests (currently **incompatible** with host parser) | Align schema + enable via Plugin Manager |

---

## Category F — TODO / Tracked Incomplete Work

| File | Line | Marker | Why | Real implementation |
|------|------|--------|-----|---------------------|
| `crates/promptlab-discovery/examples/verify_target.rs` | 15 | `TODO: use worker_count 4+ after crawler deadlock fix` | Workaround forces single worker | Fix crawler notify deadlock in `crawler.rs` |

**Search result:** `unimplemented!()` — **0** · `todo!()` — **0** · `FIXME` — **0**

---

## Category G — Not Mocks (Clarification)

These matched search terms but are **legitimate production code**, not mocks:

| Item | File | Explanation |
|------|------|-------------|
| `{{payload}}` placeholder | `promptlab-attack/src/types.rs` | Template variable in JSON body — not a mock |
| `SearchInput placeholder` prop | `src/shared/components/SearchInput.tsx` | HTML placeholder attribute |
| Embedded payload catalog | `promptlab-payload/src/library/mod.rs` | Real static payload library from `data/payloads.json` |
| Built-in attack payloads | `promptlab-attack/src/attacks/prompt_injection.rs` | Real attack content, not test fakes |
| Rule/regex evaluators | `promptlab-judge/src/evaluators/*` | Real deterministic evaluation |
| `LogOptions::bootstrap()` | `promptlab-core/src/logging.rs` | Logging init helper name, not app mock |
| `DelimiterInjection` mutator name | `docs/ATTACK.md` | Attack technique label |
| `wiremock` in `Cargo.lock` | lockfile | Test dependency only |

---

## MVP Replacement Map

What replaces mocks for the [6-step MVP flow](MVP_GAP_ANALYSIS.md):

| MVP step | Mock(s) to replace | Real module |
|----------|-------------------|-------------|
| 1. Create project | `mockProjects`, New Project placeholder, `AppState` | `promptlab-storage` + `project_create` IPC |
| 2. Add target | `mockTargets`, Add Target placeholder | `promptlab-storage` + `target_create` IPC |
| 3. Run discovery | `mockDiscoveryJobs`, Start Scan placeholder | `promptlab-discovery` via `scan_run` IPC |
| 4. Prompt injection | `mockAttackRuns`, Launch Attack placeholder | `promptlab-attack` in `scan_run` pipeline |
| 5. Evaluate | `mockFindings`, status reducer | `promptlab-judge` + `promptlab-storage` findings repo |
| 6. HTML report | `mockReports`, Generate Report placeholder | `promptlab-report` + `report_generate` IPC |

---

## Priority Order for De-mocking

| Priority | Item | Effort |
|----------|------|--------|
| P0 | Tauri `AppState` + DB + 7 IPC commands | Medium |
| P0 | Replace `AppStore` initialState hydration | Small |
| P0 | Wire project/target/scan/report buttons | Medium |
| P1 | Remove mock fallback as default when Tauri present | Small |
| P1 | Finding status → IPC persist | Small |
| P2 | Models page → real model registry | Medium |
| P2 | Plugin samples → fix manifest + orchestrator hook | Medium |
| P3 | Keep test doubles (`MockTransport`, etc.) unchanged | — |

---

## File Index (all mock-related source files)

```
PRODUCT MOCKS
  src/shared/mock/data.ts
  src/app/store/AppStore.tsx
  src/App.tsx
  src/shared/ipc/client.ts
  src-tauri/src/state.rs
  src-tauri/src/commands/mod.rs

UI PLACEHOLDERS
  src/features/projects/ProjectsPage.tsx
  src/features/targets/TargetsPage.tsx
  src/features/discovery/DiscoveryPage.tsx
  src/features/attacks/AttacksPage.tsx
  src/features/findings/FindingsPage.tsx
  src/features/reports/ReportsPage.tsx
  src/features/models/ModelsPage.tsx
  src/features/settings/SettingsPage.tsx
  src/features/dashboard/DashboardPage.tsx

TEST DOUBLES
  crates/promptlab-attack/src/transport/mock.rs
  crates/promptlab-auth/src/mock.rs
  crates/promptlab-models/src/runtime/mock.rs
  crates/promptlab-judge/src/mock_runtime.rs
  crates/promptlab-storage/src/pool.rs (test_utils)
  tests/frontend/logger.test.ts

MOCK SERVERS / FIXTURES
  scripts/discovery-test-target.py
  crates/promptlab-discovery/tests/integration.rs
  crates/promptlab-models/tests/integration.rs

SAMPLES
  plugins/_template/plugin.py
  plugins/samples/**/*
```

---

*Regenerate after IPC integration or removal of `src/shared/mock/data.ts` from production path.*
