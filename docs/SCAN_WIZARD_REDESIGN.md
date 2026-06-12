# AISec — Workflow-Driven Redesign (Scan Wizard)

> Roles: Principal PM · Principal UX · Principal Frontend Architect · Principal Tauri Architect
> Scope: UX + workflow redesign. **Reuse** existing engines (`aisec-storage`, `aisec-discovery`,
> `aisec-attack`, `aisec-judge`, `aisec-report`) and SQLite. **No mock data, no placeholders, no fake backend.**

This document is the implementation blueprint. It is grounded in the code that exists today on
`main` (after the desktop integration work): the real Tauri commands, repositories, and engine APIs.
Anything not yet present is explicitly marked **NEW** so scope is unambiguous.

---

## 0. Problem & product goal (PM)

Today the app is **page-centric**: a user must hop Projects → Targets → Discovery → Attacks → Findings → Reports,
manually carrying context between pages. There is no single "run a security test" action.

**Goal:** make the **Scan** the product's center of gravity. A user picks/creates a project and is guided through a
single **Scan Wizard** (target → discovery → attack plan → submit). Scans then execute in the **background** and are
observed in a **Scan Monitor**. Findings and Reports become **read-only outcomes** of scans.

Success metrics: time-to-first-scan ↓, page transitions per scan ↓, % scans launched via wizard ↑, no "stuck on a
spinner" screens.

---

## 1. Navigation structure

Sidebar (final):

| Item | Route | Notes |
|------|-------|-------|
| Dashboard | `/dashboard` | KPIs + active scans + recent findings |
| Projects | `/projects` | Assessment management; entry point to wizard |
| Targets | `/targets` | Target inventory + health |
| Scans | `/scans` | **Scan Monitor** (live), plus "New Scan" CTA |
| Findings | `/findings` | Read-only |
| Reports | `/reports` | Read-only |
| Settings | `/settings` | Includes **Models** subsection |

**Removed** top-level items: `Discovery`, `Attacks`, `Models`.
- Discovery and Attacks become **steps inside the Scan Wizard** (not standalone destinations).
- Models becomes a **Settings → Models** subsection (model inventory/management is configuration, not a workflow).

Sidebar component change: drop the three `NavItem`s, add `Scans`. `Models` rendered as a tab/section within
`SettingsPage` (no route removal needed beyond the sidebar entry; keep `/settings/models` as a deep-linkable tab).

---

## 2. Routing structure

React Router (v7, already in use). New/!changed routes in **bold**.

```
/                         → redirect /dashboard
/dashboard
/projects
/projects/:projectId      (NEW)  project detail (targets/scans/findings/reports tabs)
/targets
/scans                    (NEW name) Scan Monitor list
/scans/new                (NEW)  Scan Wizard
/scans/:scanId            (NEW)  Scan Monitor detail (live)
/findings
/reports
/settings
/settings/models          (NEW)  Models subsection (deep-linkable tab)
```

Removed routes: `/discovery`, `/attacks`, `/models` (redirect `/models` → `/settings/models` for back-compat).

**Deep linking:** all of `:projectId`, `:scanId` resolve their entity via IPC on mount (loading/empty/error states),
so a pasted URL works without prior in-app navigation. The wizard accepts `?projectId=` (or router `state`) to
pre-select + lock the project (Step 1 Scenario A).

---

## 3. Component hierarchy

```
<AppProviders>                      // ErrorBoundary + ToastProvider + AppStoreProvider + ScanEventsProvider(NEW)
  <AppRouter>
    <MainLayout>                    // Sidebar + TopBar + <Outlet/>
      Dashboard
      Projects
        ProjectsTable
        ProjectFormModal            // create/edit (NEW: edit)
      ProjectDetail (/:projectId)   // tabs: Targets | Scans | Findings | Reports + "New Scan"
      Targets
        TargetsTable                // + health badges
      Scans (/scans)                // Scan Monitor list
        ScanMonitorCard[]           // status/progress/findings + Pause/Resume/Stop
      ScanWizard (/scans/new)       // NEW — primary workflow
        WizardProgress              // 6-step stepper across the top
        Step1_ProjectSelect         // locked (Scenario A) | dropdown (Scenario B)
        Step2_TargetAuth            // URL + auth (None/Basic/SSO/API key)
        Step3_Discovery
          DiscoveryProgress         // live phases
          EndpointTable             // method/endpoint/confidence/source + checkbox
          ManualEndpoints           // add method+path → "Manual" badge, selectable
        Step4_AttackPlanning
          ProfileSelector           // Quick/Standard/Deep/Custom
          CategoryAccordion[]       // per category: test count + names + per-test toggle
          EstimatePanel             // estimated requests + runtime
        Step5_Submission            // "Scan Submitted" + actions (no waiting trap)
      ScanMonitorDetail (/scans/:scanId)  // live: current endpoint/test/progress + controls
      Findings                      // read-only: filter/search/export
      Reports                       // read-only: download HTML/PDF/SARIF
      Settings
        SettingsGeneral
        SettingsModels (/models)    // moved here
```

Shared/reused components already present: `Modal`, `ToastProvider/useToast`, `DataTable`, `Badge/StatusBadge/SeverityBadge`,
`PageHeader`, `Card`, `ProgressBar`, `EmptyState`.

---

## 4. State management design

Principles (already established on `main`): **all data from Tauri IPC → SQLite; async loading; loading + error states;
no mock data** (`src/shared/mock` is already deleted).

Three layers:

1. **Global entity store** — extend the existing `AppStore` (IPC-backed `projects/targets/scans/endpoints/findings/reports`,
   `loading`/`error`, async actions). Add actions: `updateProject`, `deleteTarget`, `createEndpoint` (manual),
   plus scan-control actions (below). The store remains the source of truth for list/detail pages.

2. **Wizard draft state** — a `ScanWizardProvider` (React context + reducer) scoped to `/scans/new`. Holds the
   **in-progress** selections only:
   ```ts
   type WizardDraft = {
     step: 1..6;
     projectId: string | null; projectLocked: boolean;
     target: { url; authKind: "none"|"basic"|"sso"|"api_key"; username?; password?; header?; sessionId? };
     persistedTargetId: string | null;          // set after Step 2 persists
     discoveryScanId: string | null;            // set when discovery runs
     endpoints: EndpointRow[];                   // discovered + manual, with `selected`
     profile: "quick"|"standard"|"deep"|"custom";
     categories: AttackCategory[]; disabledTests: Record<string,string[]>;
   };
   ```
   **Real persistence happens as steps complete** (not at the end): Step 2 persists the Target; Step 3 persists
   discovery endpoints (+ manual endpoints); Step 5 creates the Scan record and starts the background job. The draft
   is UI sequencing only — it is **not** a parallel data store and holds no fabricated data.

3. **Live scan events** — a `ScanEventsProvider` subscribing to Tauri events (`scan://progress`, `scan://phase`,
   `scan://finding`). Keeps a `Map<scanId, ScanProgress>` for the Monitor. Falls back to polling `scan_status` if
   events are unavailable. This is the only "real-time" state and it mirrors backend job state (no invented numbers).

---

## 5. Required IPC commands

Legend: **EXISTS** (already implemented & registered) · **NEW** (must be added) · **EXTEND** (add args/behavior).

### Projects
- `project_create(name, description)` — **EXISTS**
- `project_list()` / `project_get(id)` / `project_delete(id)` — **EXISTS**
- `project_update(id, name?, description?)` — **NEW** (storage `ProjectRepository::update` already exists; just expose it)

### Targets
- `target_create(projectId, name, targetType, descriptor)` — **EXISTS** (descriptor carries `{ url, auth }`)
- `target_list(projectId)` — **EXISTS**
- `target_get(id)` / `target_update(id, …)` / `target_delete(id)` — **NEW** (repo methods exist; expose)

### Authentication (Step 2: SSO / credentials)
- `auth_record_session(targetId, kind, credentials?)` — **NEW** — wraps `aisec-auth` Playwright flow; persists to
  `auth_profiles`/`auth_sessions` (tables exist). Returns `sessionId` to attach to the target descriptor.
  Basic/API-key need no browser (stored in descriptor); SSO launches Playwright via this command.

### Discovery (Step 3)
- `discovery_run(targetId)` — **EXISTS** (synchronous; runs real `aisec-discovery`, persists endpoints, returns report)
- `endpoint_list(scanId)` — **EXISTS**
- `endpoint_create(scanId, targetId, method, path, kind?)` — **NEW** — manual endpoint (repo `EndpointRepository`
  exists; add a create wrapper that builds the URL from target origin + path, marks `source = "manual"`)
- **EXTEND** for live progress: have discovery emit `discovery://progress` phase events (crawl / JS / API / GraphQL /
  OpenAPI). Interim: keep synchronous `discovery_run` and show indeterminate phases; full live = events (see §6).

### Scans (Step 5 + Monitor) — the core new surface
- `scan_create(projectId, targetId, name, status)` — **EXISTS** (used to persist the record)
- `scan_list(projectId)` / (NEW) `scan_list_all()` — **EXISTS / NEW** (monitor needs cross-project list)
- `scan_get(id)` — **NEW**
- `scan_start(projectId, targetId, endpointIds[], categories[], disabledTests[])` — **NEW** — creates/links the scan,
  **spawns a background job**, returns `{ scanId }` immediately (no blocking)
- `scan_status(scanId)` — **NEW** — `{ status, progress, currentEndpoint, currentTest, findingsCount, startedAt, durationMs }`
- `scan_pause(scanId)` / `scan_resume(scanId)` / `scan_stop(scanId)` — **NEW**
- Events: `scan://progress`, `scan://phase`, `scan://finding`, `scan://completed` — **NEW**

### Attacks + Judge (executed inside the scan job, not a page)
- `attack_run_prompt_injection(endpointId)` — **EXISTS** (synchronous, single endpoint, single category; the judge is
  already wired in). This is the **proof-of-concept** the background runner generalizes.
- The background `scan_start` job **reuses** `aisec-attack` `AttackExecutor::execute_category` across the selected
  categories × endpoints and `aisec-judge` `deterministic_engine().judge_deterministic` per response, persisting
  `attack_results` + `findings` (exactly as `attack_run_prompt_injection` does today). No new evaluation logic.

### Findings (read-only)
- `finding_list(scanId)` — **EXISTS**; (NEW) `finding_list_by_project(projectId)` (repo `list_by_project` exists),
  `finding_search(query)` (repo `search` exists) — expose for filter/search.
- `finding_export(scanId, format)` — **NEW** (CSV/JSON of findings) for the Findings "Export" action.

### Reports (read-only)
- `report_generate(projectId, scanId, format, kind)` — **EXISTS** (HTML proven). `format ∈ {html,pdf,json,sarif}`
- `report_list(projectId)` / `report_read(id)` / `report_export(id)` — **EXISTS**
- **Report formats are all real:** `aisec-report` implements `Html`, `Json`, `Sarif`, and `Pdf` (the PDF formatter uses
  `printpdf` + chart rendering). The only **EXTEND** needed: `report_generate` must pass the chosen `format` through
  (it already accepts it), and binary delivery — `report_read` does `read_to_string`, which is fine for HTML/JSON/SARIF
  but would corrupt **PDF**; the Reports "Download PDF" must use `report_export` (file copy, binary-safe) or a new
  binary `report_read_bytes`. So PDF/SARIF need **wiring**, not new engine code.

### Estimation (Step 4)
- Estimated requests/runtime computed **client-side** from real payload counts: `payload_count(category) × selected
  endpoints`, runtime = requests × measured avg latency. Optionally **NEW** `attack_estimate(categories, endpointIds)`
  to compute server-side from `aisec-payload` (authoritative). No fabricated numbers either way.

---

## 6. Required backend integration changes

The wizard's first four steps reuse **existing** commands. The genuinely new backend is the **scan job lifecycle**
(Steps 5 + Monitor). Summary of changes, smallest-footprint first:

1. **Expose existing repo methods** (trivial): `project_update`, `target_get/update/delete`, `scan_get`,
   `scan_list_all`, `finding_list_by_project`, `finding_search`, `endpoint_create` (manual). These are thin command
   wrappers over methods that already exist in `aisec-storage`.

2. **Scan Job Manager** (`src-tauri`, NEW module, the main work):
   - `AppState` gains `jobs: Arc<Mutex<HashMap<ScanId, JobHandle>>>` where `JobHandle` holds a `tokio::task` + a
     `CancellationToken`/pause flag + a live `ScanProgress`.
   - `scan_start` validates inputs, sets scan `status="running"`, spawns a `tokio::spawn` job that: iterates selected
     endpoints × categories, calls `aisec-attack` executor, runs `aisec-judge`, persists `attack_results` + `findings`
     (reusing today's logic from `commands/attack.rs`), updating `ScanProgress` and emitting `scan://*` events after
     each unit. Returns immediately.
   - `scan_pause/resume/stop` toggle the flag / cancel the token; the job checks between units (cooperative
     cancellation — safe, no killed mid-request).
   - On completion: scan `status="completed"`, `completed_at`, emit `scan://completed`.
   - Persistence of progress: write coarse progress into `scans.playbook_json` (or a small `scan_progress` column/JSON)
     so the Monitor survives an app restart (reads last known state, then resumes live events).

3. **Discovery in the wizard:** keep `discovery_run` synchronous for v1 (Step 3 shows phase chips + spinner). For true
   live phases, refactor `aisec-discovery` to accept a progress callback and have the command emit `discovery://progress`.
   This is optional polish; not required for a working wizard.

4. **Auth:** add `auth_record_session` wrapping `aisec-auth` (Playwright). Basic/API-key are stored in the target
   `descriptor_json`; SSO uses the recorded session. The attack executor already supports `AttackTarget.with_auth`/
   headers — feed the stored token/headers into the job.

5. **Reports PDF/SARIF:** the formatters already exist (`printpdf`-based PDF + SARIF/JSON). Only **wiring** is needed:
   pass `format` through `report_generate`, and serve PDF via the binary-safe `report_export` (or add `report_read_bytes`)
   since `report_read` is text-only.

No mock data, no fake endpoints: every number on screen (endpoints, requests, findings, progress) originates from a
real engine call or a real SQLite row.

---

## 7. Migration plan from current UI

Incremental, each phase shippable and green (`cargo test` + `tsc`/`vite`), no big-bang rewrite.

- **Phase 0 — Shell (low risk):** update Sidebar (remove Discovery/Attacks/Models, add Scans); add routes
  `/scans`, `/scans/new`, `/scans/:scanId`, `/projects/:projectId`, `/settings/models`; redirect old routes. Move the
  existing Models page under Settings. No backend changes.
- **Phase 1 — Projects:** add `project_update` command + `ProjectFormModal` edit; add `ProjectDetail` (`/:projectId`)
  with tabs reusing existing lists; wire "Create Project" success → `navigate('/scans/new', { state:{ projectId } })`.
- **Phase 2 — Wizard Steps 1–3 (reuse engines):** Step 1 (locked/dropdown), Step 2 (`target_create` + auth descriptor;
  `auth_record_session` for SSO), Step 3 (`discovery_run` + `endpoint_list` + `endpoint_create` manual + selection).
  Existing Discovery page logic is **moved** into Step 3 components.
- **Phase 3 — Wizard Step 4 + estimation:** profile presets mapped to real `AttackCategory` values, category accordion
  from `aisec-payload` test names/counts, client-side estimate. (Maps spec names → engine: "Goal Hijacking" →
  `AgentGoalHijacking`, "Indirect Injection" → prompt-injection payload subset, etc.)
- **Phase 4 — Background scan + Monitor (core):** implement Job Manager + `scan_start/status/pause/resume/stop` +
  events; Step 5 "Scan Submitted" (no waiting trap); `/scans` + `/scans/:scanId` live. Generalize the existing
  `attack_run_prompt_injection` logic into the job. Deprecate that single-shot command once the job covers it.
- **Phase 5 — Read-only outcomes:** Findings (filter/search/export via existing repo methods + `finding_export`),
  Reports (HTML now; PDF/SARIF after formatter work), Targets health badges.
- **Phase 6 — Cleanup:** remove `DiscoveryPage`/`AttacksPage` components and their routes; remove dead store actions;
  update docs.

### Risk notes
- The **Job Manager** is the only substantial new backend; everything else is wiring over existing commands/repos.
- Pause/resume must be **cooperative** (check between requests) to avoid corrupting in-flight HTTP/judge state.
- Keep `attack_run_prompt_injection` until Phase 4 lands so the app stays functional throughout.

---

## Appendix — spec → existing engine mapping

| Wizard concept | Real engine / table |
|----------------|---------------------|
| Discovery phases | `aisec-discovery` (crawler + OpenAPI/GraphQL/AI probes) |
| Endpoints (+manual) | `endpoints` table / `EndpointRepository` |
| Attack categories | `aisec_attack::AttackCategory` (PromptInjection, SystemPromptExtraction, Jailbreak, RagLeakage, MemoryPoisoning, CrossUserLeakage, AgentGoalHijacking, ToolAbuse, McpAbuse) |
| Verdict + confidence | `aisec-judge` deterministic engine (rule+regex) → `findings.evidence_json` |
| Reports | `aisec-report` (`ReportFormat::{Html,Pdf,Json,Sarif}`) → `reports` table |
| Auth (Basic/SSO/API key) | `aisec-auth` (Playwright) → `auth_profiles`/`auth_sessions` |
| Persistence | SQLite via `aisec-storage` repositories |
