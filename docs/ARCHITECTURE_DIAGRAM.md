# AISec — Architecture Diagrams

> Visual reference for system structure, data flow, and job lifecycle.  
> Version: 0.1.0 · Stack: Tauri 2 + React 19 + Rust workspace + SQLite

---

## HIGH LEVEL ARCHITECTURE

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              AISec Desktop Application                          │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                 │
│  ┌──────────────────────────────────────┐    ┌──────────────────────────────┐  │
│  │         FRONTEND (WebView)           │    │     TAURI IPC BRIDGE         │  │
│  │  React 19 · TypeScript · Vite        │    │  invoke() · 32 commands      │  │
│  │                                      │    │                              │  │
│  │  ┌─────────┐  ┌──────────────────┐  │    │  health · project_*          │  │
│  │  │  Pages  │  │  Scan Wizard (6) │  │◄──►│  target_* · scan_*           │  │
│  │  │Dashboard│  │  Project→Target  │  │    │  discovery_* · endpoint_*    │  │
│  │  │Projects │  │  →Discovery→Plan │  │    │  attack_* · scan_start       │  │
│  │  │Findings │  │  →Submit→Results │  │    │  report_* · auth_record_*    │  │
│  │  └────┬────┘  └────────┬─────────┘  │    └──────────────┬───────────────┘  │
│  │       │                │             │                   │                  │
│  │  ┌────▼────────────────▼─────────┐  │                   │                  │
│  │  │  AppStore (Context+Reducer)   │  │                   │                  │
│  │  │  + wizard sessionStorage      │  │                   │                  │
│  │  └──────────────┬────────────────┘  │                   │                  │
│  │                 │                    │                   │                  │
│  │  ┌──────────────▼────────────────┐  │                   │                  │
│  │  │  shared/ipc (typed wrappers)  │──┼───────────────────┘                  │
│  │  └───────────────────────────────┘  │                                      │
│  └──────────────────────────────────────┘                                      │
│                                                                                 │
│  ┌──────────────────────────────────────────────────────────────────────────┐  │
│  │                    BACKEND (Rust — src-tauri + crates)                   │  │
│  │                                                                          │  │
│  │  ┌─────────────┐   ┌─────────────────┐   ┌─────────────────────────┐  │  │
│  │  │  commands/  │──►│  AppState       │──►│  aisec-storage          │  │  │
│  │  │  projects   │   │  · Database     │   │  SQLite + repositories  │  │  │
│  │  │  domain     │   │  · ScanJobMgr   │   └───────────┬─────────────┘  │  │
│  │  │  discovery  │   │  · auth config  │               │                │  │
│  │  │  attack     │   └────────┬────────┘               │                │  │
│  │  │  scan       │            │                          │                │  │
│  │  │  auth       │            ▼                          ▼                │  │
│  │  └─────────────┘   ┌────────────────────────────────────────────┐     │  │
│  │                    │           ENGINE CRATES (workspace)           │     │  │
│  │                    │                                           │     │  │
│  │                    │  aisec-discovery   aisec-attack           │     │  │
│  │                    │  aisec-payload     aisec-judge            │     │  │
│  │                    │  aisec-report      aisec-auth             │     │  │
│  │                    │  aisec-models*     aisec-fingerprint*     │     │  │
│  │                    │  aisec-plugin-host* aisec-core            │     │  │
│  │                    │  (* = library only, not fully wired)    │     │  │
│  │                    └────────────────────────────────────────────┘     │  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
│                                                                                 │
│  ┌──────────────────────────────┐   ┌──────────────────────────────────────┐  │
│  │  PERSISTENCE                 │   │  EXTERNAL RUNTIMES                   │  │
│  │  · {app_data}/aisec.db       │   │  · HTTP targets (reqwest)            │  │
│  │  · {app_data}/reports/       │   │  · Playwright (Node + Chromium)      │  │
│  │  · {app_data}/auth-vault/    │   │  · Local GGUF (llama.cpp)*           │  │
│  └──────────────────────────────┘   └──────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### Layer summary

| Layer | Technology | Role |
|-------|------------|------|
| UI | React pages + shared components | Operator workflows |
| State | AppStore + wizard sessionStorage | Client-side cache and draft persistence |
| IPC | `@tauri-apps/api` invoke | Typed command boundary |
| Commands | `src-tauri/src/commands/` | Validation, orchestration, DTO mapping |
| Engines | `crates/aisec-*` | Discovery, attack, judge, report, auth |
| Storage | `aisec-storage` + SQLite | Single source of truth |

---

## FRONTEND FLOW

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                    UI                                       │
│  Pages · Modals · Scan Wizard Steps · Shared Components (Button, Table…)   │
│                                                                             │
│  User action: click "Run Discovery" / "Start Scan" / "New Project"         │
└─────────────────────────────────────┬───────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                                  STATE                                      │
│                                                                             │
│  ┌─────────────────────────────┐    ┌─────────────────────────────────┐  │
│  │  AppStore (global)          │    │  Wizard session (local)         │  │
│  │  · useReducer + Context     │    │  · sessionStorage key:          │  │
│  │  · projects, targets, scans │    │    aisec:scan-wizard (v2)       │  │
│  │  · findings, reports,       │    │  · currentStep, targetForm,     │  │
│  │    endpoints                │    │    discovery, attackPlan        │  │
│  │  · backendConnected         │    │  · survives tab refresh         │  │
│  │  · actions.refresh()        │    └─────────────────────────────────┘  │
│  │  · actions.createProject()  │                                          │
│  │  · actions.runDiscovery()   │    ┌─────────────────────────────────┐  │
│  └──────────────┬──────────────┘    │  Ephemeral UI state             │  │
│                 │                   │  · form errors, loading flags     │  │
│                 │                   │  · useScanStatuses poll map       │  │
│                 │                   └─────────────────────────────────┘  │
└─────────────────┼───────────────────────────────────────────────────────────┘
                  │
                  │  IPC call via actions.* or direct import
                  ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                                    IPC                                      │
│                                                                             │
│  shared/ipc/invoke.ts          → invokeCommand(name, args)                  │
│  shared/ipc/client.ts          → listProjects, startScan, runDiscovery…    │
│  shared/ipc/projects.ts        → project CRUD                               │
│  shared/ipc/auth.ts            → authRecordSessionStart/Finish              │
│                                                                             │
│  Tauri maps JS camelCase ↔ Rust snake_case                                  │
│  Errors → CommandError → toAppError() → toast / inline message              │
└─────────────────────────────────────┬───────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                                  BACKEND                                    │
│                                                                             │
│  lib.rs invoke_handler                                                      │
│       │                                                                     │
│       ▼                                                                     │
│  commands/*_op(state, …)  ──►  AppState.repositories()  ──►  Engine crates │
│       │                              │                                      │
│       ▼                              ▼                                      │
│  dto.rs (response)              SQLite read/write                          │
│       │                                                                     │
│       └──────────────────────────► JSON response to frontend               │
│                                      │                                      │
└──────────────────────────────────────┼──────────────────────────────────────┘
                                       │
                                       ▼
                              AppStore.refresh()
                              dispatch SET_DATA
                                       │
                                       ▼
                              UI re-renders with
                              updated projects/findings/…
```

### Bootstrap sequence

```
App mount
   │
   ├─► healthCheck() + getAppInfo()     ──► SET_BACKEND { connected, version }
   │
   └─► AppStore.refresh()
          │
          ├─► listProjects()
          ├─► listTargets (per project)
          ├─► listScans (per project)
          ├─► listFindingsAll()
          ├─► listReportsAll()
          └─► listEndpoints (per scan)
                 │
                 └─► SET_DATA → pages render
```

### Browser-only mode (`npm run dev`)

```
UI renders ──► IPC invoke fails ──► backendConnected = false
                                      │
                                      └─► AppStore stays empty (no mock fixtures)
                                          TopBar shows "Mock mode"
```

---

## SCAN EXECUTION FLOW

End-to-end path from operator intent to deliverable report.

```
┌──────────┐
│ PROJECT  │  Wizard Step 1 · ProjectsPage
│          │  IPC: project_create / project_list
│          │  DB:  projects
└────┬─────┘
     │  project_id
     ▼
┌──────────┐
│  TARGET  │  Wizard Step 2 · TargetFormFields + PlaywrightRecordPanel
│          │  IPC: target_create
│          │       auth_record_session_start / finish (User/Pass, SSO)
│          │  DB:  targets (descriptor_json: url, auth kind, credentials)
│          │  FS:  auth-vault/storageState (Playwright sessions)
└────┬─────┘
     │  target_id
     ▼
┌──────────┐
│DISCOVERY │  Wizard Step 3 · DiscoveryStep
│          │  IPC: discovery_run(target_id)
│          │       endpoint_list(scan_id) · endpoint_create (manual)
│          │  Engine: aisec-discovery
│          │    · static probes (AI, GraphQL, OpenAPI paths)
│          │    · HTTP crawler (depth 2, max 25 pages)
│          │  DB:  scans (status: running → completed)
│          │       endpoints (url, kind, method, confidence)
│          │  UI:  operator selects endpoint_ids[]
└────┬─────┘
     │  endpoint_ids[] + attack plan
     ▼
┌──────────┐
│  ATTACK  │  Wizard Step 4–5 · AttackPlanStep → scan_start
│  PLAN    │  IPC: scan_start { projectId, targetId, endpointIds,
│          │              profile, categories[], disabledTests[] }
│          │  Engine: aisec-attack (per endpoint × category)
│          │    · PayloadRunner → aisec-payload/payloads.json
│          │    · HttpTransport → real HTTP to endpoint.url
│          │    · apply_descriptor_auth → headers from target
│          │  BG:   ScanJobManager (async tokio task)
│          │  DB:  scans (playbook_json.progress)
│          │       attack_results (every attempt)
└────┬─────┘
     │  HTTP response + payload per attempt
     ▼
┌──────────┐
│  JUDGE   │  Inside run_category_on_endpoint (attack.rs)
│          │  Engine: aisec-judge
│          │    · judge_deterministic() [production path]
│          │      RuleBasedEvaluator + RegexEvaluator
│          │    · judge() with LLM [available, not configured in app]
│          │  Output: JudgeVerdict { vulnerable, confidence, severity }
│          │  DB:  evaluated_json on attack_results
└────┬─────┘
     │  if verdict.vulnerable
     ▼
┌──────────┐
│ FINDINGS │  Wizard Step 6 · ResultsStep · FindingsPage
│          │  Created in attack.rs when judge confirms vulnerability
│          │  DB:  findings (title, severity, category, evidence_json)
│          │       findings_fts (full-text index, auto-synced)
│          │  UI:  listFindingsAll · filter · severity badges
└────┬─────┘
     │  scan_id + project_id
     ▼
┌──────────┐
│  REPORT  │  ResultsStep · ReportsPage · GenerateReportModal
│          │  IPC: report_generate → report_read / report_export
│          │  Engine: aisec-report
│          │    · ReportDataBuilder ← findings from SQLite
│          │    · HTML | PDF | JSON | SARIF formatters
│          │  FS:  {app_data}/reports/{filename}
│          │  DB:  reports (file_path, metadata_json)
└──────────┘
```

### Attack inner loop (per endpoint × category)

```
scan_start background job
        │
        ▼
for each endpoint_id in endpointIds
        │
        ▼
for each category in categories
        │
        ├─► update ScanProgress (current_endpoint, current_test)
        │
        ├─► run_category_on_endpoint()
        │        │
        │        ├─► AttackExecutor.execute_category()
        │        │        └─► HTTP probe + category-specific evaluation
        │        │
        │        └─► for each attempt:
        │                 JudgeEngine.judge_deterministic()
        │                 persist attack_result
        │                 if vulnerable → create finding
        │
        ├─► persist playbook_json.progress
        │
        └─► check cancel / pause flags
        │
        ▼
scan status → completed | failed | cancelled
```

### Ad-hoc paths (outside wizard)

```
DiscoveryPage ──► discovery_run ──► same discovery block above

AttacksPage   ──► attack_run_prompt_injection(endpoint_id)
                      └── single category, new scan row, sync IPC
```

---

## DATABASE FLOW

### Entity-relationship diagram

```
                              ┌─────────────┐
                              │  projects   │
                              │─────────────│
                              │ id (PK)     │
                              │ name        │
                              │ description │
                              └──────┬──────┘
                                     │
         ┌───────────────────────────┼───────────────────────────┐
         │                           │                           │
         ▼                           ▼                           ▼
┌─────────────────┐        ┌─────────────────┐        ┌─────────────────┐
│    targets      │        │     scans       │        │    reports      │
│─────────────────│        │─────────────────│        │─────────────────│
│ id (PK)         │        │ id (PK)         │        │ id (PK)         │
│ project_id (FK) │◄──┐    │ project_id (FK) │        │ project_id (FK) │
│ descriptor_json │   │    │ target_id (FK)──┼──►     │ scan_id (FK)────┼──► scans
│ target_type     │   │    │ status          │        │ format, file_path│
└────────┬────────┘   │    │ playbook_json   │        └─────────────────┘
         │            │    └────────┬────────┘
         │            │             │
         │            │    ┌────────┼────────┬────────────────┐
         │            │    │        │        │                │
         │            │    ▼        ▼        ▼                ▼
         │            │ ┌──────┐ ┌────────┐ ┌─────────────┐ ┌──────────────┐
         │            └─┤endpts│ │findings│ │attack_results│ │  (progress  │
         │              │──────│ │────────│ │──────────────│ │  in playbook)│
         │              │scan  │ │scan_id │ │ scan_id (FK) │ └──────────────┘
         └──────────────►target│ │project │ │ payload_id   │
                        │ url   │ │target  │ │ probe_id     │
                        │ kind  │ │severity│ │ success      │
                        └──────┘ │category│ │ response_json│
                                 │evidence│ │ evaluated_json│
                                 └───┬────┘ └──────────────┘
                                     │
                                     ▼
                            ┌─────────────────┐
                            │  findings_fts   │  (FTS5 virtual)
                            │  title, desc    │
                            └─────────────────┘

┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│ auth_profiles   │────►│ auth_sessions   │     │ auth_recordings │
│─────────────────│     │─────────────────│     │─────────────────│
│ project_id (FK)?│     │ profile_id (FK) │     │ profile_id (FK) │
│ method          │     │ storage_state   │     │ steps_json      │
│ config_json     │     │ cookies, tokens │     │ storage_state   │
└─────────────────┘     └─────────────────┘     └─────────────────┘

┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   payloads      │     │     models      │     │    plugins      │
│ (custom library)│     │ (GGUF registry) │     │ (plugin host)   │
│ project_id (FK)?│     │ not UI-wired    │     │ not UI-wired    │
└─────────────────┘     └─────────────────┘     └─────────────────┘
```

### Cascade rules

```
DELETE project  ──CASCADE──►  targets, scans, findings, reports, auth_profiles
DELETE scan     ──CASCADE──►  findings, attack_results, endpoints
DELETE target   ──SET NULL──► scans.target_id, findings.target_id, endpoints.target_id
```

### Read patterns by feature

| Feature | Primary reads | Primary writes |
|---------|---------------|----------------|
| Dashboard | projects, findings (aggregated) | — |
| Projects | projects, targets, scans | projects |
| Scan wizard | projects, targets | targets, scans, endpoints |
| Scan job | endpoints, targets | attack_results, findings, scans.playbook |
| Findings | findings (+ FTS) | — (status UI local only) |
| Reports | findings, scans, projects | reports + filesystem |
| Auth recording | — | auth_profiles, auth_sessions, auth_recordings, vault files |

---

## BACKGROUND JOB FLOW

AISec uses **two execution models**: synchronous IPC handlers and async background tasks.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         EXECUTION MODEL LEGEND                              │
│                                                                             │
│  [SYNC IPC]   Command blocks until complete; UI shows spinner               │
│  [BG TASK]    Command returns immediately; progress polled separately       │
│  [SYNC GEN]   File generation inline; no job manager entry                  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Discovery jobs

Discovery runs **synchronously inside the IPC handler** (not registered in ScanJobManager).

```
UI: runDiscovery(targetId)
        │
        ▼
IPC: discovery_run  [SYNC IPC — blocks 5–60s typical]
        │
        ├─► CREATE scan  (status: running, name: "Discovery: …")
        │
        ├─► DiscoveryEngine.discover(seed_url)
        │        ├─ static path probes
        │        └─ HTTP crawler (worker_count: 1)
        │
        ├─► BULK INSERT endpoints
        │
        └─► UPDATE scan  (status: completed | failed)
        │
        ▼
Return DiscoveryRunDto { scan, endpoints[], stats }
        │
        ▼
AppStore.refresh()
```

```
Scan row lifecycle (discovery):

  pending ──► running ──► completed
                    └──► failed (error in playbook_json)
```

### Scan jobs

Multi-category attack scans use **true background jobs** via `ScanJobManager`.

```
UI: startScan({ projectId, targetId, endpointIds, categories, profile })
        │
        ▼
IPC: scan_start  [BG TASK — returns immediately]
        │
        ├─► Validate project, target, endpoint ownership
        │
        ├─► CREATE scan (status: running, playbook: profile + categories)
        │
        ├─► ScanJobManager.register(scan_id)
        │        · cancel: AtomicBool
        │        · paused: AtomicBool
        │        · progress: Mutex<ScanProgress>
        │
        ├─► tokio::spawn(run_scan_job(...))
        │
        └─► RETURN ScanStartDto { scan_id }
        │
        ▼
UI polls scan_status(scan_id) every ~2s  (useScanStatuses hook)
        │
        ├─► reads ScanJobManager progress (in-memory)
        └─► fallback: playbook_json.progress from SQLite (after restart)
```

```
Scan job control:

  scan_pause(scan_id)   ──► paused.store(true)   progress.status = "paused"
  scan_resume(scan_id)  ──► paused.store(false)  progress.status = "running"
  scan_stop(scan_id)    ──► cancel.store(true)   job exits → status "cancelled"
```

```
Background worker inner state:

  ScanProgress {
    status:        running | paused | completed | failed | cancelled
    completed:     N of (endpoints × categories)
    total:         endpoints.len() × categories.len()
    findings:      cumulative count
    current_endpoint, current_test
  }
        │
        └── persisted to scans.playbook_json.progress after each unit
```

```
Scan row lifecycle (attack):

  running ──► completed   (all units done, findings ≥ 0 or partial errors)
          ──► failed      (errors, zero findings)
          ──► cancelled   (scan_stop)
          ──► paused      (transient, via job manager)
```

### Report jobs

Report generation is **synchronous** — no background job manager.

```
UI: generateReport(projectId, scanId, format, kind)
        │
        ▼
IPC: report_generate  [SYNC GEN — blocks 1–10s]
        │
        ├─► LOAD project, scan, findings[] from SQLite
        │
        ├─► ReportDataBuilder.build(scan metadata + findings)
        │
        ├─► ReportingEngine.generate(kind, format, input)
        │        └─► write file to {app_data}/reports/
        │
        ├─► CREATE reports row (file_path, status: completed)
        │
        └─► RETURN ReportDto
        │
        ▼
report_read(id)     ──► read file contents into ReportContentDto
report_export(id)   ──► return filesystem path for OS save dialog
```

```
Report row lifecycle:

  (created directly as completed — no pending/running state in practice)
```

### Job comparison table

| Job type | Async? | Job manager? | Progress API | Persisted progress |
|----------|--------|--------------|--------------|-------------------|
| Discovery | No (sync IPC) | No | UI spinner only | scan.status + stats in response |
| Attack scan | Yes (tokio spawn) | ScanJobManager | `scan_status` poll | playbook_json.progress |
| Report | No (sync IPC) | No | UI exporting flag | reports.file_path |

---

## DATA PERSISTENCE FLOW

How data moves from user input through engines to durable storage and back to the UI.

### 1. Write path (mutation)

```
┌──────────┐     ┌──────────┐     ┌──────────────┐     ┌─────────────┐     ┌──────────┐
│ Operator │────►│    UI    │────►│  IPC command │────►│  Repository │────►│  SQLite  │
│  input   │     │  form /  │     │  *_op()      │     │  .create()  │     │  INSERT  │
│          │     │  wizard  │     │  validation  │     │  .update()  │     │  UPDATE  │
└──────────┘     └──────────┘     └──────────────┘     └─────────────┘     └──────────┘
                                              │
                                              ▼
                                    Engine side effects
                                    · HTTP requests (attack)
                                    · Playwright vault files (auth)
                                    · Report files on disk (reports/)
```

### 2. Read path (load / refresh)

```
┌──────────┐     ┌──────────────┐     ┌─────────────┐     ┌──────────┐     ┌──────────┐
│  SQLite  │────►│  Repository  │────►│  dto.rs     │────►│   IPC    │────►│ AppStore │
│  SELECT  │     │  .list()     │     │  From<Row>  │     │  JSON    │     │ mappers  │
│          │     │  .get()      │     │  RFC3339 ts │     │  response│     │ SET_DATA │
└──────────┘     └──────────────┘     └─────────────┘     └──────────┘     └────┬─────┘
                                                                                  │
                                                                                  ▼
                                                                            UI re-render
```

### 3. Full scan data lifecycle

```
PHASE          INPUT                    STORED AS                    OUTPUT
─────────────────────────────────────────────────────────────────────────────────
Project        name, description   →   projects row            →   project_id
Target         url, auth config    →   targets.descriptor_json →   target_id
                                       auth_profiles/sessions*       (* Playwright only)
Discovery      target_id           →   scans (discovery type)  →   scan_id
                                       endpoints[]                   endpoint_ids[]
Attack plan    categories, profile →   (client sessionStorage) →   scan_start request
Scan run       endpoint×category   →   attack_results[]        →   evaluated_json
                                       findings[] (if vuln)          finding_ids[]
Results        scan_id             →   (read findings)         →   UI tables
Report         scan_id, format     →   reports + FS file       →   export path
```

### 4. Dual persistence: wizard vs database

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  WIZARD DRAFT (sessionStorage)          │  DATABASE (SQLite)              │
│─────────────────────────────────────────│─────────────────────────────────│
│  Current step, form fields              │  Committed entities only        │
│  Selected endpoint IDs (pre-submit)     │  targets after Step 2 proceed   │
│  Attack plan UI state                   │  endpoints after discovery_run  │
│  Lost on tab close                      │  scans after discovery/scan_start│
│  Version gate (v2)                      │  findings after attack+judge    │
└─────────────────────────────────────────┴─────────────────────────────────┘
                          │
                          └── On scan_start success: submittedScanId links
                              wizard Step 6 to DB scan row
```

### 5. File-system persistence (outside SQLite)

```
{app_data_dir}/
├── aisec.db                    ← all relational data
├── reports/
│   └── {scan}-{kind}-{format}.{ext}   ← report_generate output
├── auth-vault/
│   └── {session}/storageState.json    ← Playwright recorded sessions
└── logs/                       ← tracing output (if configured)

Release bundle (read-only):
resources/playwright/           ← Node + playwright + Chromium (Tauri resources)
```

### 6. Evidence chain (audit trail)

```
HTTP response
     │
     ▼
attack_results.response_json     { status, body, duration_ms }
     │
     ▼
attack_results.evaluated_json    { attack_evaluation, judge: JudgeVerdict }
     │
     ▼ (if vulnerable)
findings.evidence_json           { payload, confidence, indicators,
                                   response_excerpt, judge }
     │
     ▼
reports (via ReportDataBuilder)  formatted HTML/PDF/JSON/SARIF
```

---

## Quick reference: command → storage mapping

| IPC command | SQLite tables touched | Filesystem |
|-------------|----------------------|------------|
| `project_create` | projects | — |
| `target_create` | targets | — |
| `auth_record_session_*` | auth_profiles, auth_sessions, auth_recordings | auth-vault/ |
| `discovery_run` | scans, endpoints | — |
| `endpoint_create` | endpoints | — |
| `scan_start` | scans, attack_results, findings | — |
| `attack_run_prompt_injection` | scans, attack_results, findings | — |
| `report_generate` | reports | reports/ |
| `scan_status` | scans (read playbook) | — |

---

*See also: [PROJECT_CURRENT_STATE.md](./PROJECT_CURRENT_STATE.md) · [PROJECT_FILE_INDEX.md](./PROJECT_FILE_INDEX.md) · [ARCHITECTURE.md](./ARCHITECTURE.md)*
