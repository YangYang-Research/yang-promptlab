# PromptLab MVP Checklist

**Goal:** A user can open the desktop app and complete a full pentest loop on a target URL — from project creation through HTML report export.

**MVP flow:**

```
Open app → Create project → Add target URL → Crawl → Discover endpoints
    → Prompt injection tests → Evaluate → HTML report
```

**Last updated:** 2026-06-10  
**Baseline:** See `docs/STATUS.md` for module classifications

---

## MVP Success Criteria

| # | Step | Done when |
|---|------|-----------|
| 1 | Open desktop app | App launches; UI loads; backend connected |
| 2 | Create project | Project persisted in SQLite; visible in UI after restart |
| 3 | Add target URL | Target linked to project; URL stored and validated |
| 4 | Crawl target | HTTP crawl runs against seed URL; pages fetched |
| 5 | Discover endpoints | Endpoints listed and persisted (REST, OpenAPI, AI routes, links) |
| 6 | Execute prompt injection | Payloads sent to discovered AI/API endpoint; responses captured |
| 7 | Evaluate results | Each response scored; findings created with severity |
| 8 | Generate HTML report | HTML file written to disk; openable from UI |

---

## Step-by-Step Checklist

### Step 1 — Open desktop app

| | Item | Status |
|---|------|--------|
| ✅ | Tauri shell builds and launches | **Done** |
| ✅ | React UI renders (9 pages, layout, routing) | **Done** |
| ✅ | `health` / `app_info` IPC | **Done** |
| ⬜ | App opens SQLite on startup (`~/.promptlab/promptlab.db`) | **Missing** |
| ⬜ | `AppState` holds `Database` handle | **Missing** — `state.rs` only stores log guard |
| ⬜ | Frontend hydrates from backend (not mock-only mode) | **Missing** |
| ⬜ | Global error surfacing for IPC failures | **Partial** — falls back to mock silently |

**Missing work**
- [ ] Add `promptlab-storage` to `src-tauri/Cargo.toml`
- [ ] Initialize `Database::connect` in Tauri `setup`
- [ ] Store `Database` in `AppState` (behind `Mutex` or `Arc`)
- [ ] Resolve app data directory (Tauri `path` API → `~/.promptlab/`)
- [ ] Replace mock bootstrap with real project list fetch (Step 2 dependency)

---

### Step 2 — Create project

| | Item | Status |
|---|------|--------|
| ✅ | `ProjectRepository` + `CreateProject` model | **Done** (`promptlab-storage`) |
| ✅ | Projects UI table (read mock data) | **Done** |
| ⬜ | IPC command `project_create` | **Missing** |
| ⬜ | IPC command `project_list` | **Missing** |
| ⬜ | IPC command `project_get` / `project_delete` | **Missing** (MVP: create + list minimum) |
| ⬜ | Frontend create-project form/modal | **Missing** — no create action wired |
| ⬜ | Store dispatch → IPC → reload projects | **Missing** |
| ⬜ | Typed IPC bindings in `src/shared/ipc/` | **Missing** |

**Missing work**
- [ ] `src-tauri/src/commands/project.rs` — validate name, call `repos.projects().create()`
- [ ] Register commands in `lib.rs` invoke handler
- [ ] Serialize `Project` → JSON DTO for frontend
- [ ] `projectCreate()` / `projectList()` in `src/shared/ipc/client.ts`
- [ ] Add "New Project" modal on `ProjectsPage` with name + description fields
- [ ] `AppStore` actions: `LOAD_PROJECTS`, `ADD_PROJECT` (from IPC response)
- [ ] Remove hard-coded mock projects on successful backend connect

---

### Step 3 — Add target URL

| | Item | Status |
|---|------|--------|
| ✅ | `TargetRepository` + `CreateTarget` model | **Done** |
| ✅ | Targets UI table (read mock data) | **Done** |
| ⬜ | IPC `target_create` (project_id, name, url, type) | **Missing** |
| ⬜ | IPC `target_list` (by project) | **Missing** |
| ⬜ | URL validation before save | **Missing** in app layer (discovery has `validate_target_url`) |
| ⬜ | Frontend add-target form | **Missing** — Targets page has no create flow |
| ⬜ | Map URL → `descriptor_json` (e.g. `{"url":"..."}`) | **Missing** in IPC handler |

**Missing work**
- [ ] `target_create` command — validate URL scheme (http/https), require project_id
- [ ] `target_list` command — filter by `project_id`
- [ ] Frontend modal: name, URL, type (`web` / `llm_api` for MVP)
- [ ] Wire `TargetsPage` "Add Target" button (currently non-functional)
- [ ] Persist `target_type` compatible with downstream attack routing (`llm_api` vs `web`)

---

### Step 4 — Crawl target

| | Item | Status |
|---|------|--------|
| ✅ | `DiscoveryEngine::discover(seed_url)` | **Done** (`promptlab-discovery`) |
| ✅ | `Crawler` — BFS, depth/page limits, same-origin | **Done** |
| ✅ | `HttpClient` with timeout/retry | **Done** |
| ✅ | Discovery UI page (display mock jobs) | **Done** (UI only) |
| ⬜ | IPC `scan_start` or `discovery_run` | **Missing** |
| ⬜ | Create `Scan` record (status: running → completed) | **Missing** |
| ⬜ | Link scan to `target_id` + `project_id` | **Missing** |
| ⬜ | Progress events to frontend | **Missing** |
| ⬜ | "Start Scan" button invokes backend | **Missing** — `DiscoveryPage` button is inert |

**Missing work**
- [ ] MVP run orchestrator (can live in `src-tauri` initially): accept `target_id`, load target URL, spawn discovery
- [ ] `scan_create` + `scan_update_status` repository calls
- [ ] Tauri async command calling `DiscoveryEngine::discover(&target.url)`
- [ ] Optional: Tauri events (`discovery://progress`) with pages fetched / percent
- [ ] Frontend: bind "Start Scan" to IPC; show real progress instead of mock jobs
- [ ] Handle crawl errors (timeout, blocked host) → update scan status `failed`

---

### Step 5 — Discover endpoints

| | Item | Status |
|---|------|--------|
| ✅ | Static probes (OpenAPI, GraphQL, AI paths) | **Done** |
| ✅ | `DiscoveredEndpoint` types (`EndpointKind`) | **Done** |
| ✅ | `DiscoveryReport` with deduped endpoints | **Done** |
| ⬜ | Persist endpoints after discovery | **Missing** — no endpoints table; store in scan metadata or findings |
| ⬜ | IPC `discovery_results` / include in scan response | **Missing** |
| ⬜ | UI endpoint list view | **Missing** — "View Results" button inert |
| ⬜ | Map discovery → attack target selection | **Missing** |

**Missing work (storage design)**
- [ ] **Option A (MVP-fast):** Store `DiscoveryReport` JSON in `scans.playbook_json` or new `scan_results_json` column via migration
- [ ] **Option B:** New `discovered_endpoints` table — preferred for query/filter later
- [ ] IPC returns endpoint list: `{ url, kind, method, confidence }`
- [ ] Frontend results panel: table of endpoints with kind badges
- [ ] **Endpoint → attack target resolver:** pick first `AiEndpoint` or `RestApi` POST URL; fallback to OpenAPI-derived chat path; MVP heuristic documented

---

### Step 6 — Execute prompt injection tests

| | Item | Status |
|---|------|--------|
| ✅ | `PromptInjectionAttack` with 3 default payloads | **Done** |
| ✅ | `AttackExecutor::execute_category(PromptInjection, ctx)` | **Done** |
| ✅ | `HttpTransport` for LLM API POST | **Done** |
| ✅ | `AttackTarget::llm_api(url)` body template | **Done** |
| ✅ | Attacks UI page (mock runs) | **Done** (UI only) |
| ⬜ | Run prompt injection after discovery | **Missing** — no pipeline glue |
| ⬜ | Build `AttackContext` from discovered endpoint | **Missing** |
| ⬜ | Persist `attack_results` rows | **Missing** |
| ⬜ | Store raw request/response evidence | **Missing** |
| ⬜ | UI trigger / auto-run after discovery | **Missing** |

**Missing work**
- [ ] Orchestrator step: after discovery, select attack URL (see Step 5 resolver)
- [ ] Construct `AttackContext { scan_id, probe_id, target: AttackTarget::llm_api(url), ... }`
- [ ] Call `AttackOrchestrator` or `executor.execute_category(PromptInjection, &ctx)` only (MVP: single category, not all 9)
- [ ] Map `AttackExecutionResult` → `CreateAttackResult` + storage insert
- [ ] For **web/chatbot targets** (no JSON API): MVP scope decision required — either restrict MVP to `llm_api` targets with known POST endpoint, or add minimal chatbot transport (out of scope unless Playwright wired)
- [ ] IPC `attack_run` or fold into unified `scan_run` pipeline
- [ ] Frontend: show attack phase status on Discovery/Attacks page

**MVP scope note:** Prompt injection against a **raw website URL** without an AI API endpoint will not work with current `HttpTransport` + JSON body template. MVP should either:
- require target type `llm_api` with discovered `/v1/chat/completions`-style endpoint, or
- add explicit "API endpoint URL" field on target (user-provided)

---

### Step 7 — Evaluate results

| | Item | Status |
|---|------|--------|
| ✅ | Inline `PromptInjectionAttack::evaluate` | **Done** (basic indicators) |
| ✅ | `JudgeEngine` — rule + regex + LLM consensus | **Done** (`promptlab-judge`) |
| ✅ | `CreateFinding` + `FindingRepository` | **Done** |
| ✅ | Findings UI (mock list, status toggle) | **Done** (local state only) |
| ⬜ | Invoke `JudgeEngine` on each attack response | **Missing** |
| ⬜ | Persist findings to SQLite | **Missing** |
| ⬜ | Fix judge false negative (`"API key:"` regex gap) | **Missing** — 1 integration test fails |
| ⬜ | IPC `finding_list` (by scan/project) | **Missing** |
| ⬜ | UI loads findings from backend | **Missing** |

**Missing work**
- [ ] After each attack attempt: build `JudgeRequest { probe_id, attack_category, payload, response_text, context }`
- [ ] Call `JudgeEngine::judge_deterministic()` for MVP (no local LLM required)
- [ ] Map `JudgeVerdict` → `CreateFinding { title, severity, category, description, evidence_json }`
- [ ] Insert via `repos.findings().create()`
- [ ] Fix regex pattern to match spaced `"api key:"` OR lower consensus threshold for single-evaluator agreement
- [ ] `finding_list` IPC; hydrate Findings page from DB
- [ ] Deduplicate findings per scan (same category + similar evidence)

**MVP decision:** Use `promptlab-judge` deterministic path (not attack inline evaluate) for Step 7 to satisfy "Evaluate results" as a distinct stage.

---

### Step 8 — Generate HTML report

| | Item | Status |
|---|------|--------|
| ✅ | `ReportingEngine::generate(Html, input)` | **Done** |
| ✅ | `HtmlFormatter` | **Done** |
| ✅ | `ReportDataBuilder` | **Done** |
| ✅ | `ReportRepository` + `CreateReport` | **Done** |
| ✅ | Reports UI (mock list) | **Done** (UI only) |
| ⬜ | Build `ReportInput` from scan findings | **Missing** |
| ⬜ | IPC `report_generate` (scan_id, format=html) | **Missing** |
| ⬜ | Save report metadata + file path to DB | **Missing** |
| ⬜ | Open/export file from UI | **Missing** |
| ⬜ | Tauri shell permission to write/read report dir | **Missing** |

**Missing work**
- [ ] Load findings + scan + project + target for `scan_id`
- [ ] Build `ReportInput` via `ReportDataBuilder`
- [ ] `ReportingEngine::new(app_data_dir/reports)`
- [ ] `report_generate` command → returns `{ path, filename, bytes_len }`
- [ ] Insert `CreateReport` row with file path
- [ ] Frontend "Generate Report" on Reports page or post-scan CTA
- [ ] Tauri `shell.open` or `dialog` to reveal HTML file in Finder/Explorer
- [ ] Add Tauri capabilities for filesystem write to reports directory

---

## Cross-Cutting Missing Work

Infrastructure required across all steps:

### A. Tauri integration spine

| Task | Priority |
|------|----------|
| Add workspace deps: `promptlab-storage`, `promptlab-discovery`, `promptlab-attack`, `promptlab-judge`, `promptlab-report`, `tokio` | P0 |
| Extend `AppState` with `Database` + config paths | P0 |
| Command module layout: `commands/{project,target,scan,finding,report}.rs` | P0 |
| Unified error mapping: `PromptLabError` → `CommandError` | P0 |
| Async command support (`async fn` + `tauri::State`) | P0 |

### B. MVP run pipeline (orchestration)

No crate connects the full flow today. Minimum viable orchestrator (can start in `src-tauri`):

```
scan_run(target_id)
  1. Load target + create scan (running)
  2. discovery.discover(target.url)
  3. persist endpoints
  4. resolve attack URL
  5. attack.execute_category(PromptInjection)
  6. for each attempt → judge.judge_deterministic → finding.create
  7. scan.status = completed
  8. return { scan_id, findings_count, endpoints_count }
```

| Task | Priority |
|------|----------|
| Implement `scan_run` (single async Tauri command) | P0 |
| Scan status machine: `pending → running → completed \| failed` | P0 |
| Error handling: partial results on failure | P1 |
| Cancel/abort (optional for MVP) | P2 |

### C. Frontend ↔ backend

| Task | Priority |
|------|----------|
| IPC client functions for all MVP commands | P0 |
| Replace mock data hydration on connect | P0 |
| Loading/error states per page | P1 |
| Post-scan flow: Discovery → Findings → Report CTA | P1 |

### D. Schema / migrations

| Task | Priority |
|------|----------|
| Store discovery results (JSON column or `discovered_endpoints` table) | P0 |
| `attack_results.evidence_json` populated with request/response | P0 |
| Optional: `003_scan_pipeline.sql` migration | P0 |

### E. Bug fixes blocking MVP quality

| Issue | Crate | Blocks |
|-------|-------|--------|
| `ScanRepository` trait not in scope in storage tests | `promptlab-storage` | CI confidence (not runtime) |
| Judge `regex_and_rules_agree_on_secret` fails | `promptlab-judge` | Step 7 accuracy |
| Plugin manifest drift | `promptlab-plugin-host` | Not MVP-critical |

### F. Explicitly out of MVP scope

- Plugin SDK / custom plugins
- All 9 attack categories (MVP: prompt injection only)
- LLM-based judge (use deterministic judge)
- Playwright / chatbot UI attacks
- PDF/SARIF export (HTML only)
- Auth sessions / authenticated targets
- Model manager / local llama.cpp
- Fingerprint provider detection (nice-to-have post-discovery)
- Run Console streaming / WebSocket events
- License, updates, vault encryption

---

## Implementation Phases

### Phase 0 — Foundation (blocks everything)

- [ ] Database in Tauri `AppState`
- [ ] `project_create`, `project_list`
- [ ] `target_create`, `target_list`
- [ ] Frontend create forms + IPC hydration

**Exit:** User creates project + target; data survives app restart.

### Phase 1 — Discovery

- [ ] `scan_run` phase 1: discovery only
- [ ] Persist + return endpoints
- [ ] Discovery UI shows real results

**Exit:** User crawls target URL and sees endpoint list.

### Phase 2 — Attack + Judge

- [ ] Endpoint → `AttackTarget` resolver
- [ ] Prompt injection execution
- [ ] Judge + finding persistence
- [ ] Findings UI from DB

**Exit:** User sees real findings with severity after scan.

### Phase 3 — Report

- [ ] `report_generate` (HTML)
- [ ] Reports UI + open file

**Exit:** User exports HTML report for completed scan.

---

## MVP Definition of Done

- [ ] Fresh install: open app, no mock data when backend connected
- [ ] Create project "MVP Test" → appears in list
- [ ] Add target `https://<test-server>/` (or local wiremock fixture)
- [ ] Start scan → crawl completes, ≥1 endpoint shown
- [ ] Prompt injection runs against resolved API endpoint
- [ ] ≥0 findings persisted (pass or fail — pipeline must complete)
- [ ] Generate HTML report → file exists on disk, opens in browser
- [ ] Restart app → project, target, findings, report metadata still present
- [ ] `cargo test -p promptlab-judge --test integration` passes (Step 7 gate)

---

## Quick Reference: What Exists vs What's Missing

| Layer | Exists | Missing for MVP |
|-------|--------|-----------------|
| **UI** | Pages, layout, mock store | Forms, IPC calls, real data |
| **Tauri** | 2 commands, logging | DB, domain commands, orchestrator |
| **Storage** | Full schema + repos | App connection, discovery result storage |
| **Discovery** | Engine + crawler | Trigger + persist + UI results |
| **Attack** | Prompt injection + HTTP | Pipeline glue, target resolution |
| **Judge** | Engine (deterministic) | Pipeline invoke, regex fix, finding map |
| **Report** | HTML generator | Input assembly, IPC, file open |

---

## Estimated Work Summary

| Area | New work size |
|------|---------------|
| Tauri commands + AppState | ~400–600 LOC |
| MVP orchestrator (`scan_run`) | ~200–350 LOC |
| Frontend IPC + forms + hydration | ~300–500 LOC |
| Migration / endpoint storage | ~50–100 LOC + SQL |
| Judge fix + tests | ~20–50 LOC |
| **Total** | **~1–1.5k LOC** (focused MVP, no new crates) |

---

*Track progress by checking boxes above. When all Phase 0–3 exit criteria pass, MVP is complete.*
