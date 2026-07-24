# AISec — Complete Product & Architecture Audit

**Audit type:** Read-only, evidence-only  
**Audience:** Architect with zero prior knowledge of AISec  
**Date:** 2026-06-13  
**Repository:** `yang-aisec-private`  
**Stack:** Tauri 2 desktop app — React/TypeScript frontend (`src/`), Rust backend (`src-tauri/`, `crates/`)

> **Rule:** Every capability is **NOT IMPLEMENTED** unless cited with file path and line evidence.  
> **Screenshots:** Not captured in this audit (no screenshot artifacts in repository).

---

# 1. Executive Summary

## What AISec Is (from code)

AISec is a **desktop AI security testing application** that:
- Manages **projects**, **targets**, and **scans**
- Runs **discovery** (crawl/API fingerprinting) against web targets
- Executes **prompt-injection and related attack categories** via a harness
- **Judges** responses (rules, regex, optional LLM)
- Generates **HTML/PDF/SARIF reports** from SQLite findings
- Manages **local GGUF models** (llama-server) and **third-party cloud models**
- Provides an **AI Runtime** UI for local vs third-party inference routing

**Evidence:** Route map `src/app/router/AppRouter.tsx:81-225`; scan pipeline `src-tauri/src/commands/scan.rs:106-695`; discovery `src-tauri/src/commands/discovery.rs:158+`.

## Overall Architecture (one paragraph)

The UI is a React SPA inside a Tauri webview. All backend operations go through **Tauri invoke commands** into `AppState`, which holds a SQLite database, a single `RuntimeManager` (llama-server supervisor), `LocalModelManager`, `HarnessFactory`, and `PluginManager`. Domain logic lives in **17 Rust workspace crates**. AI inference for scans uses **`judge_config.json`**; the AI Runtime page uses a **separate** `ai_inference_settings.json`. There is **no unified AiService**.

## Implemented Modules

| Module | Status | Evidence |
|--------|--------|----------|
| Projects CRUD | ✅ | `commands/projects.rs`, `ProjectsPage.tsx` |
| Targets CRUD | ✅ | `commands/domain.rs`, `TargetsPage.tsx` |
| Discovery engine + UI | ✅ | `aisec-discovery`, `discovery_run` |
| Fingerprint engine | ✅ | `aisec-fingerprint`, `fingerprint_service.rs` |
| Scan wizard (6 steps) | ✅ | `ScanWizardPage.tsx`, `wizardSteps.ts:16-53` |
| Linear scan execution | ✅ | `run_scan_job` `scan.rs:106-318` |
| Agent scan mode | ⚠️ Partial | `run_agent_scan_job` — LLM planner/generator gaps |
| Attack harness (HTTP/Playwright) | ✅ | `aisec-harness`, `harness_runtime.rs` |
| Judge engine | ✅ | `aisec-judge`, `attack.rs:151-199` |
| Attack planner (wizard) | ✅ | `planner_generate`, `AttackPlanStep.tsx` |
| Payload generator | ✅ | `generator_generate`, `scan.rs:155-188` |
| Reports (non-AI) | ✅ | `aisec-report`, `report_generate` |
| Models vault + downloads | ✅ | `ModelsPage.tsx`, `commands/models.rs` |
| AI Runtime (local llama.cpp) | ✅ | `AIRuntimePage.tsx`, `commands/runtime.rs` |
| Third-party models | ✅ | `models_save_third_party`, third-party UI |
| Judge Provider config page | ✅ | `JudgeProviderPage.tsx`, `judge_config.json` |
| Plugins host | ⚠️ Partial | Discovery/attack/judge wired; report plugins not |
| Auth session recording | ✅ | `commands/auth.rs`, `PlaywrightRecordPanel` |
| Dashboard | ✅ | `DashboardPage.tsx` |
| Settings + security audit | ✅ | `SettingsPage.tsx`, `security_audit` |

## Partially Implemented Modules

| Module | Gap | Evidence |
|--------|-----|----------|
| Agent scan LLM | Planner always deterministic; generator passes `None` LLM | `agent_service.rs:96-99,112-114` |
| AI config unification | Two JSON config files | `judge_config.json` vs `ai_inference_settings.json` |
| Attack categories (UI) | Only `prompt_injection` enabled on Attacks page | `AttacksPage.tsx:18-23` |
| Plugin report hooks | `PluginType::Report` exists, not invoked | `aisec-plugin-host/types.rs` |
| Findings deep-link | `?scanId=` linked but not read | `FindingsPage.tsx` (no `useSearchParams` for scanId) |
| Dashboard runtime card | No health metric; load once on mount | `DashboardPage.tsx:52-54` |

## Missing Modules (not found in source)

| Module | Evidence |
|--------|----------|
| `AiService` / inference gateway | Grep: no symbol in `.rs`/`.ts` |
| `PromptGenerator` (named) | No crate/module/IPC |
| AI-powered reports | `aisec-report` — rule-based only |
| LLM streaming | `"stream": false` in `llama_cpp_runtime.rs:229` |
| Named pipe / Unix socket runtime IPC | Grep: 0 matches |
| Global hotkey system | No `useHotkey`; only Escape in `Modal.tsx:23-27` |
| Feature flag framework | No `featureFlag` / env flags beyond `VITE_LOG_LEVEL` |
| Zustand / Redux | Only `AppStore` Context + `ToastProvider` Context |

## Major Technical Debt

1. Dual AI configuration (`judge_config` vs `ai_inference_settings`)
2. Triplicated LLM backend wiring (`factory.rs`, `planner.rs`, `generator.rs`)
3. Tauri `runtime` ↔ `models` command module cycle
4. Legacy Ollama naming post-llama.cpp migration
5. Agent mode not wired for Local LLM planner/generator
6. Unused IPC exports (`getRuntimeStatus`, `installModel`, etc.)
7. Discovery crawler pinned to `worker_count: 1` (deadlock comment `discovery.rs:220-223`)

## Overall Completion %

| Area | % | Basis |
|------|---|--------|
| UI pages & navigation | **92%** | 17/17 routes implemented; minor filter/deep-link gaps |
| Core pentest workflow (wizard → scan → findings) | **85%** | End-to-end wired; agent LLM partial |
| AI Runtime product surface | **88%** | Full lifecycle UI; judge page still separate |
| Unified AI architecture | **40%** | No gateway; split config |
| Backend commands & persistence | **90%** | 90 Tauri commands; SQLite migrations complete |
| **Overall product** | **~78%** | Weighted across above |

---

# 2. Application Navigation

**Router:** `HashRouter` in `src/app/router/AppRouter.tsx:1+`  
**Layout:** `MainLayout` → `Sidebar` + `TopBar` + `<Outlet />` (`src/app/layout/MainLayout.tsx`)  
**Sidebar nav:** `src/app/router/nav.ts:9-23`

**Global chrome (all pages):**
- **Sidebar:** section links; findings critical badge (`Sidebar.tsx:37-39`); collapse toggle → `TOGGLE_SIDEBAR`
- **TopBar:** route title; global search → `SET_SEARCH` (`TopBar.tsx:29-34`); Connected/Mock indicator (`TopBar.tsx:37-49`)
- **Hotkeys:** None global. Escape closes `Modal` (`Modal.tsx:23-27`), `ActionsDropdown` (`ActionsDropdown.tsx:82-87`)
- **Feature flags:** None. Closest: attack categories disabled with "(soon)" (`AttacksPage.tsx:18-23`)
- **Permissions:** Desktop app; no role-based UI gates found
- **Screenshots:** Not available in repo

---

## Page: Dashboard

| Field | Value |
|-------|-------|
| **Route** | `/` (`AppRouter.tsx:81-87`) |
| **Purpose** | Workspace overview: stats, severity chart, activity, active jobs, projects, AI Runtime summary |
| **Entry points** | Sidebar "Dashboard"; post-login default index route |
| **Exit points** | Links to `/projects`, `/runtime`, `/findings` |
| **Toolbar actions** | **New Project** → `/projects` with `{ openNewProject: true }` (`DashboardPage.tsx:62-67`) |
| **IPC** | `getRuntimeConfiguration()` when `backendConnected` (`DashboardPage.tsx:37-49`) |
| **Store** | `stats`, `findings`, `activity`, `discoveryJobs`, `attackRuns`, `projects`, `backendConnected` |
| **Child components** | `StatCard`×3, `AiRuntimeDashboardCard`, `Card`×4, `SeverityBadge`, `ProgressBar`, `Badge` |
| **Parent** | `MainLayout` |
| **Lifecycle** | `useEffect` loads runtime config on `backendConnected` change (`DashboardPage.tsx:52-54`) |
| **Render conditions** | Runtime card hidden when not connected; Active Jobs empty state (`DashboardPage.tsx:151-154`) |

---

## Page: Projects

| Field | Value |
|-------|-------|
| **Route** | `/projects` |
| **Purpose** | List, create, delete projects |
| **Toolbar** | `RefreshButton` → `actions.refresh()`; **New Project** |
| **Dialogs** | `NewProjectModal` — open via button or `location.state.openNewProject` (`ProjectsPage.tsx:41-47`) |
| **IPC** | `project_list`, `project_create`, `project_delete` via AppStore |
| **Store** | `projects`, `ui.searchQuery`, `loading`, `error`, `actions` |
| **View modes** | Table / list via `ContentToolbar` + `useViewPreference("projects")` |

---

## Page: Project Details

| Field | Value |
|-------|-------|
| **Route** | `/projects/:projectId` |
| **Purpose** | Project info, targets summary, findings, reports |
| **Toolbar** | **New Scan** `?projectId=`; `ActionsDropdown` Edit / Delete |
| **Dialogs** | `EditProjectModal` |
| **IPC** | `actions.deleteProject()` |
| **Exit** | Back via `PageHeader backTo` |

---

## Page: Targets

| Field | Value |
|-------|-------|
| **Route** | `/targets` |
| **Purpose** | Flat target list with scan context |
| **Query** | `?projectId=` filter (`TargetsPage.tsx:65`) |
| **Toolbar** | `RefreshButton`; **Add Target** |
| **Dialogs** | `AddTargetModal` |
| **IPC** | `target_list`, `target_create` via store |

---

## Page: Target Details

| Field | Value |
|-------|-------|
| **Route** | `/targets/:targetId` |
| **Purpose** | Target metadata + recent scans |
| **Toolbar** | **New Scan**, **View Project** |
| **IPC** | None direct (reads store) |

---

## Page: Scans

| Field | Value |
|-------|-------|
| **Route** | `/scans` |
| **Purpose** | Monitor attack/agent scans (filters scan names) |
| **Toolbar** | `RefreshButton`; **New Scan** → `/scans/new` |
| **IPC** | `scan_pause`, `scan_resume`, `scan_stop`, `getScanStatus` |
| **Children** | `ScanMonitorCard`, `ScanHistoryCard` |

---

## Page: Scan Wizard

| Field | Value |
|-------|-------|
| **Route** | `/scans/new` |
| **Purpose** | 6-step scan configuration and submission |
| **Query** | `?projectId=` locks project step |
| **Toolbar** | **Cancel** → `/scans`; footer Back / Next / Start Scan / View Result / Done |
| **IPC** | `getProject`, `startScan`, `actions.createTarget`, step-specific discovery/planner/generator |
| **Persistence** | `localStorage` key `aisec:scan-wizard` (`wizardState.ts:15-16`) |
| **Parent components** | `WizardStepper`, step components |
| **Poll** | 3s refresh after submit (`ScanWizardPage.tsx` ~L138-142) |

---

## Page: Scan Details

| Field | Value |
|-------|-------|
| **Route** | `/scans/:scanId` |
| **Purpose** | Read-only scan config, execution, results, report export |
| **IPC** | `getScan`, `getTarget`, `getScanStatus`, `generateAndExportScanReport` |
| **Render** | Progress when running; report export when attack scan + playbook |

---

## Page: Discovery

| Field | Value |
|-------|-------|
| **Route** | `/discovery` |
| **Purpose** | Discovery history; trigger discovery per target |
| **Toolbar** | `RefreshButton`; `ContentToolbar` table/tree |
| **IPC** | `actions.runDiscovery(targetId)` → `discovery_run` |
| **View modes** | Table / tree |

---

## Page: Discovery Details

| Field | Value |
|-------|-------|
| **Route** | `/discovery/:scanId` |
| **Purpose** | Single discovery run: fingerprint, endpoints, stats |
| **IPC** | `getScan`; `actions.refresh()` |
| **Toolbar** | `RefreshButton` |

---

## Page: Attacks

| Field | Value |
|-------|-------|
| **Route** | `/attacks` |
| **Purpose** | Ad-hoc prompt-injection on discovered endpoints |
| **Toolbar** | Endpoint `Select`, category `Select`, **Launch Attack** |
| **IPC** | `actions.runPromptInjection` → `attack_run_prompt_injection` |
| **Disabled** | Categories other than `prompt_injection` show "(soon)" (`AttacksPage.tsx:18-23`) |

---

## Page: Findings

| Field | Value |
|-------|-------|
| **Route** | `/findings` |
| **Purpose** | Read-only findings table with filters |
| **Toolbar** | Inline filter card: `SearchInput`, project/scan selects, severity chips |
| **IPC** | `actions.refresh()` only |
| **Gap** | Links use `?scanId=` but page does not read it from URL |

---

## Page: Reports

| Field | Value |
|-------|-------|
| **Route** | `/reports` |
| **Purpose** | Generate HTML/PDF/SARIF + download stored reports |
| **IPC** | `generateReport`, `exportReport` via `reportDownloads.ts` |
| **Sections** | Export reports table + Stored reports archive |

---

## Page: Judge Provider

| Field | Value |
|-------|-------|
| **Route** | `/judge` |
| **Purpose** | Configure hybrid judge (mode, local/remote, connectivity tests) |
| **IPC** | `judge_config_get/save`, `judge_test_connectivity`, `judge_test_model`, `listModels` |
| **Toolbar** | **Save Judge Config** |
| **Render** | Local fields when `local_llm` or `consensus`; remote hint for `remote_llm` |

---

## Page: AI Runtime

| Field | Value |
|-------|-------|
| **Route** | `/runtime` |
| **Purpose** | Third-party vs local runtime; install/manage llama.cpp; load models |
| **IPC** | Full `runtime_*` suite + `runtime_set_inference_route` + install progress event |
| **Toolbar** | `RefreshButton`, `ModeToggle` (when configured) |
| **Dialogs** | Load model confirm, Unload model confirm (`AIRuntimePage.tsx:994-1055`) |
| **Render** | `RuntimeModePicker` when `not_configured`; third-party vs local sections |

---

## Page: Models

| Field | Value |
|-------|-------|
| **Route** | `/models` |
| **Purpose** | Local/cloud model vault: catalog download, import, test, remove |
| **IPC** | 20+ `models_*` commands (see Appendix) |
| **Toolbar** | `RefreshButton`, **Add Model** |
| **Dialogs** | `AddModelModal`; local test confirm `Modal` |
| **Location state** | `openAddModel`, `openAddModelTab`, `editModelId` (`ModelsPage.tsx:105-137`) |

---

## Page: Plugins

| Field | Value |
|-------|-------|
| **Route** | `/plugins` |
| **Purpose** | Enable/disable discovery/attack/judge plugins |
| **IPC** | `plugins_list`, `plugins_refresh`, `plugins_enable/disable`, `plugins_info` |
| **Render** | Offline card when `!backendConnected` |

---

## Page: Settings

| Field | Value |
|-------|-------|
| **Route** | `/settings` |
| **Purpose** | Client preferences + troubleshooting + security |
| **Tabs** | general, troubleshooting, security, paths, about (`SettingsPage.tsx:18-24`) |
| **IPC** | `models_registry_diagnostics`, `security_audit`, `security_migrate_secrets` |
| **Store** | `settings` persisted in reducer (theme, offlineMode, autoJudge, telemetry, dirs) |

---

# 3. UI Component Inventory

**Location:** `src/shared/components/` (18 component files + `index.ts` barrel)

| Component | Purpose | Key props | Consumers (approx.) | State ownership |
|-----------|---------|-----------|---------------------|-----------------|
| **PageHeader** | Page title, back, actions | `title`, `description?`, `backTo?`, `actions?` | ~15 pages | Parent |
| **Button** | Primary actions | `variant`, `size`, `disabled`, `onClick` | Widespread | Parent |
| **RefreshButton** | Refresh with 3s spin + toast | `onClick`, `loading?`, `error?` | ~11 pages | Internal spin state |
| **Card** | Surface container | `padding`, `className` | Widespread | Parent |
| **StatCard** | Dashboard metric | `label`, `value`, `hint?`, `accent?` | Dashboard | Parent |
| **DataTable** | Tabular data | `columns`, `rows`, `keyField`, `onRowClick?` | ~12 pages | Parent |
| **ListCard** | Card list item | `title`, `metadata`, `actions`, `onClick?` | Projects, Targets, Scans | Parent |
| **Modal** | Dialog overlay | `open`, `title`, `onClose`, `footer?`, `size?` | ~6 modals | Parent `open` |
| **ActionsDropdown** | ⋮ menu | `items[]`, `disabled?` | Project details, Model registry table | Internal open state |
| **Pagination** | Page controls | `page`, `totalPages`, `onPageChange`, ranges | List pages | Parent |
| **ContentToolbar** | Page size + view toggle | `pageSize`, `viewMode`, filters? | List pages | Parent + hooks |
| **EmptyState** | Zero-data placeholder | `title`, `description`, `action?` | Scans, Discovery, etc. | Parent |
| **Badge / SeverityBadge / StatusBadge** | Labels | `variant` / `severity` / `status` | Widespread | Parent |
| **ConnectivityStatus** | Status + dot | `label` | AI Runtime, Models | Parent |
| **ProgressBar** | Progress display | `value`, `label?` | Dashboard, scans | Parent |
| **SearchInput** | Filter input | `value`, `onChange` | TopBar, Findings | Store/parent |
| **Select** | Native select styled | HTML select attrs | Forms, filters | Parent |
| **IconButton / Icons** | Icon actions | `ariaLabel`, children | Discovery, dropdowns | Parent |
| **ViewModeToggle** | Table/list switch | `mode`, `onChange` | Registry pages | Parent |

**Missing variants (not found in code):** primary destructive `Button` pattern for bulk actions; `Tooltip` component; `Tabs` shared component (Settings uses inline tabs); `Breadcrumb` beyond `PageHeader backTo`.

**Hooks (not components but shared UI state):**
- `useViewPreference` — localStorage view mode
- `usePageSizePreference` — localStorage page size
- `useAiInferenceRoute` — runtime configuration
- `useRuntimeModelLoading` — global model load poll

---

# 4. Scan Wizard Audit

**Orchestrator:** `src/features/scans/ScanWizardPage.tsx`  
**Steps defined:** `src/features/scans/wizardSteps.ts:16-53`  
**Session persistence:** `localStorage` `aisec:scan-wizard` v2 (`wizardState.ts:15-16`)

## Step 1 — Project

| Field | Evidence |
|-------|----------|
| **Purpose** | Select project for scan | `wizardSteps.ts:17-22` |
| **Input** | Project `Select` | `ProjectStep.tsx` |
| **Validation** | `projectId` non-empty | `wizardSteps.ts:71-72` |
| **Persistence** | Wizard session localStorage | `wizardState.ts` |
| **IPC** | None on this step alone | — |
| **Back** | Disabled on step 1 | `ScanWizardPage.tsx` footer logic |
| **Next** | Requires `projectId` | `isStepComplete` case 1 |
| **Locked** | `?projectId=` query pre-selects | `ScanWizardPage.tsx:46` |

## Step 2 — Target & Authentication

| Field | Evidence |
|-------|----------|
| **Purpose** | Target URL + auth descriptor | `wizardSteps.ts:24-28` |
| **Input** | `TargetFormFields` + optional `PlaywrightRecordPanel` | `TargetStep.tsx` |
| **Validation** | `isTargetFormValid` | `wizardState.ts:4`, `wizardSteps.ts:74-75` |
| **Persistence** | On Next: `actions.createTarget` | `ScanWizardPage.tsx` (persist target) |
| **Authentication** | Playwright session recording | `PlaywrightRecordPanel.tsx` |
| **Back** | Returns to step 1; retains session | wizard session |
| **Edge** | Invalid URL blocks Next | `targetDescriptor` validation |

## Step 3 — Discovery

| Field | Evidence |
|-------|----------|
| **Purpose** | Run discovery, fingerprint, select endpoints | `wizardSteps.ts:30-34` |
| **IPC** | `actions.runDiscovery`, `listEndpoints`, `createEndpoint`, `updateEndpoint` | `DiscoveryStep.tsx` |
| **Validation** | `discoveryCompleted && selectedCount > 0` | `wizardSteps.ts:76-77` |
| **Fingerprint** | Backend auto-fingerprints during `discovery_run` | `discovery.rs:283-294` |
| **Progress** | Phase badges + `ProgressBar` | `DiscoveryStep.tsx` |
| **Failure** | Error surfaced in step UI; user can retry discovery | `DiscoveryStep.tsx` |
| **Manual endpoints** | Form to add endpoint paths | `DiscoveryStep.tsx` |

## Step 4 — Attack Planning

| Field | Evidence |
|-------|----------|
| **Purpose** | Profile, categories, planner/generator preview | `wizardSteps.ts:36-40` |
| **IPC** | `planner_generate`, `generator_generate` | `AttackPlanStep.tsx:210-283` |
| **Validation** | `attackPlan.categories.length > 0` | `wizardSteps.ts:78-79` |
| **Modes** | Planner: deterministic/local_llm; Generator: static_pack/template_mutation/local_llm | `AttackPlanStep.tsx` |
| **Agent mode** | Toggle `agentMode` + `maxAgentAttempts` | `wizardState.ts:37-38` |
| **Persistence** | Stored in wizard session `attackPlan`, `attackPlanUi` | `wizardState.ts:28-39` |
| **Note** | Planner output is **preview only** — scan uses UI-selected categories in playbook | `scan.rs` playbook_json |

## Step 5 — Scan Submission

| Field | Evidence |
|-------|----------|
| **Purpose** | Review config; start background scan | `wizardSteps.ts:42-46` |
| **IPC** | `startScan` with playbook JSON | `ScanWizardPage.tsx:261-271` |
| **Progress** | `SubmitStep` + `ScanConsole` listens `scan-progress` event | `SubmitStep.tsx`, `events.rs:7` |
| **Validation** | `submittedScanId` set after start | `wizardSteps.ts:80-81` |
| **Auto-save** | N/A — scan row created in SQLite on start | `scan.rs:571-588` |
| **Retry** | User can navigate back before submit only | step gating |

## Step 6 — Results

| Field | Evidence |
|-------|----------|
| **Purpose** | Findings summary + report export | `wizardSteps.ts:48-52` |
| **IPC** | `generateAndExportScanReport` | `ResultsStep.tsx` |
| **Report** | HTML/PDF/SARIF via `reportDownloads.ts` | `ResultsStep.tsx:6-10` |
| **Links** | To `/findings?scanId=`, `/scans/:id` | `ResultsStep.tsx` |
| **Disabled** | Export when zero findings | `ResultsStep.tsx:158` |
| **Recovery** | User can return to wizard via session if scan id stored | localStorage |

**Wizard-wide:**
- **Cancellation:** Cancel button → `/scans` (does not stop running scan if already submitted)
- **Resume:** `loadWizardSession` / `saveWizardSession` in `wizardState.ts`
- **Permissions:** Requires Tauri backend for IPC steps

---

# 5. Dashboard Audit

**File:** `src/features/dashboard/DashboardPage.tsx`  
**Refresh:** Runtime card loads once on mount / `backendConnected` — **no interval** (`DashboardPage.tsx:52-54`)

| Card | Value source | Calculation | Navigation |
|------|--------------|-------------|------------|
| **Projects** `StatCard` | `stats.projects` | `computeDashboardStats` from store data (`AppStore.tsx` + `shared/stats`) | — |
| **Targets** `StatCard` | `stats.targets` | Same | — |
| **Open Findings** | `stats.openFindings` | Same; critical hint from severity counts | — |
| **AI Runtime** | `getRuntimeConfiguration()` | `AiRuntimeDashboardCard` shows mode, status, runtime/model or provider | Click → `/runtime` |
| **Findings by Severity** | `findings` array | Count per severity; bar width vs max | — |
| **Recent Activity** | `activity` | `deriveActivity` (`shared/dashboardDerived`) | — |
| **Active Jobs** | `discoveryJobs`, `attackRuns` | `deriveDiscoveryJobs`, `deriveAttackRuns`; `ProgressBar` for running | — |
| **Projects list** | `projects` top 3 | Slice + link Manage → `/projects` | `/projects` |

**Missing metrics (not in code):** runtime health on dashboard; model name for third-party mode on card; scan success rate; plugin status.

**Current issues:** Stale runtime card until manual navigation or reconnect; no `RefreshButton` on dashboard.

---

# 6. Runtime Audit

## Configuration

| Artifact | Path | Evidence |
|----------|------|----------|
| AI inference route | `{data_dir}/ai_inference_settings.json` | `ai_inference_settings.rs:92-93` |
| Runtime install manifest | `{data_dir}/runtime/manifest.json` | `manifest.rs:66-67` |
| Hardware profile | `{data_dir}/runtime/hardware.json` | `hardware.rs:41-42` |
| Runtime DTO (UI) | `RuntimeConfigurationDto` | `commands/runtime.rs:85-97` |
| Low-level config | `aisec_runtime::RuntimeConfig` | `config.rs:8-17` |
| Env overrides | `AISEC_LLAMA_BASE_URL`, `HOST`, `PORT` | `config.rs:44-50` |

## Lifecycle

| State | Evidence |
|-------|----------|
| States enum | `RuntimeLifecycleState` `state.rs:8-20` |
| Bootstrap (no start) | `manager.rs:121-160`, `embedded_runtime.rs:28-47` |
| Install/repair | `runtime_install`, `runtime_repair` |
| Start/stop/restart/delete | `manager.rs:227-336` |
| Load/unload model | `runtime_load_model`, `runtime_unload_model` |
| Auto-resume on startup | `resume_local_runtime_on_startup` when local route + selected model (`embedded_runtime.rs:65-145`) |
| Shutdown | `lib.rs:44-54` `stop_runtime` on app exit |
| Health watch | `runtime_watch.rs` — periodic health, auto-restart |

## Communication

| Mechanism | Status | Evidence |
|-----------|--------|----------|
| **HTTP client → llama-server** | ✅ IMPLEMENTED | `llama_cpp_runtime.rs` spawn + `POST /completion`, `GET /health` |
| **Named pipe** | ❌ NOT IMPLEMENTED | Grep: 0 matches |
| **Unix domain socket** | ❌ NOT IMPLEMENTED | Grep: 0 matches |
| **Tauri IPC** | ✅ | 17 `runtime_*` commands `lib.rs:236-252` |
| **Tauri events** | ✅ | `runtime-install-progress` `events.rs:9` |
| **In-memory logs** | ✅ Ring buffer | `aisec-runtime/src/logs.rs` — not persisted to disk as runtime logs |

## Install / Repair / Detection

| Feature | IPC | Evidence |
|---------|-----|----------|
| Install | `runtime_install` | `AIRuntimePage.tsx` |
| Repair | `runtime_repair` | same |
| Hardware detect | `hardware_refresh`, startup detect | `lib.rs:123-125` |
| Health check | `runtime_health` | `manager.rs:376` |
| Benchmark | `runtime_benchmark` | `manager.rs:386` |
| Logs | `runtime_logs` | `manager.rs:403` |

## Missing (not in code)

- Inference gateway API on `RuntimeManager` (`chat`/`infer`)
- Named pipe / Unix socket transport
- Persistent runtime log files (only app `logs/` for tracing)
- ROCm detection in UI (hardcoded "No" `AIRuntimePage.tsx:792`)

---

# 7. Models Audit

**UI:** `ModelsPage.tsx`, `ModelRegistrySection.tsx`  
**Backend:** `commands/models.rs`, `aisec-models` crate

| Capability | Status | Evidence |
|------------|--------|----------|
| **Registry list** | ✅ | `models_list`, `registry.json` |
| **Catalog browse** | ✅ | `models_browse`, `resources/models.json` |
| **Download start/pause/resume/cancel** | ✅ | `models_download_*` |
| **Verify download** | ✅ | `models_download_retry_verify` |
| **Import GGUF** | ✅ | `models_import_gguf`, file picker |
| **Import ZIP** | ✅ | `models_import_zip` |
| **Third-party save/edit/test** | ✅ | `models_save_third_party`, etc. |
| **Remove** | ✅ | `models_remove` |
| **Test connection (3rd party)** | ✅ | `models_test_connection` |
| **Test inference (local)** | ✅ | `models_test_inference` |
| **Test embeddings** | ✅ IPC exists; **UI not wired** | `models_test_embeddings` registered `lib.rs:231` |
| **Install (legacy)** | ✅ IPC; **UI uses download flow** | `models_install` |
| **Vault path/stats** | ✅ | `models_vault_path`, `models_vault_stats` |
| **Activation for runtime** | ✅ | `runtime_load_model` + `ai_inference_settings.selected_model_id` |
| **Activation for judge** | ⚠️ Separate | `judge_config` `vault_model_id` |
| **Export model** | ❌ NOT FOUND | No IPC |
| **Update model metadata** | ⚠️ Third-party edit only | `models_third_party_edit_form` |
| **Storage** | `{data_dir}/models/registry.json`, `.credentials/*.enc`, GGUF files | `registry.rs`, `model_vault.rs` |

---

# 8. AI Audit

| Capability | Status | Evidence |
|------------|--------|----------|
| **Judge — rules** | ✅ | `evaluators/rule.rs` |
| **Judge — regex** | ✅ | `evaluators/regex.rs` |
| **Judge — LLM** | ✅ | `evaluators/llm.rs`, `ModelRolePool` |
| **Judge modes** | ✅ | deterministic, local_llm, remote_llm, consensus `types.rs:48-53` |
| **Judge config** | ✅ | `judge_config.json`, `JudgeProviderPage` |
| **Attack Planner** | ✅ deterministic + local LLM | `aisec-planner`, `planner_generate` |
| **Payload Generator** | ✅ static + mutation + local LLM | `aisec-generator`, `generator_generate` |
| **Prompt Generator (named)** | ❌ | Inline prompts in planner/generator LLM modules |
| **Fingerprint (AI stack)** | ✅ | `aisec-fingerprint` — provider rules, not LLM |
| **Reports AI** | ❌ | `aisec-report/recommendations.rs` rule-based |
| **Conversation** | ❌ | No multi-turn scan conversation state |
| **Prompt templates** | ⚠️ | `aisec-judge/prompts.rs` for evaluation roles only |
| **Model selection** | ⚠️ Split | AI Runtime settings vs judge vault id |
| **Runtime usage** | ⚠️ | Local: supervisor + `EmbeddedModelProvider`; remote: `RemoteLlmBackend` |
| **Direct LLM calls** | ✅ | Via `LlmBackend::complete`, `InferenceRuntime` |
| **Structured output** | ⚠️ | JSON parse from LLM responses in planner/generator/judge evaluators |
| **Retry** | ✅ Agent retry policy; discovery HTTP retry | `aisec-agent/retry.rs`, `aisec-discovery/retry.rs` |
| **Streaming** | ❌ | `"stream": false` `llama_cpp_runtime.rs:229` |
| **Telemetry** | ❌ | Settings toggle only; no export pipeline |
| **Cache** | ⚠️ | `runtime_config_cache`, regex cache fingerprint |
| **Tool calling** | ❌ | Fingerprint detects `tool_calls`; no LLM tool runtime |

---

# 9. Scan Engine Audit

## Lifecycle (`scan_start` → complete)

1. **Validate** project, target, endpoints — `scan.rs:523-561`
2. **Create scan row** + `playbook_json` — `scan.rs:571-588`
3. **Register job** `ScanJobManager` — `scan.rs:609-619`
4. **Spawn task:** agent vs linear — `scan.rs:640-695`
5. **Linear:** optional `generate_payloads_for_scan_job` — `scan.rs:155-188`
6. **Loop** endpoints × categories → `run_category_on_endpoint` — `attack.rs`
7. **Harness execute** → **Judge** → findings SQLite — `attack.rs:146-199`
8. **Update status** completed/failed/cancelled — `scan.rs:282-308`
9. **Emit** `app-data-changed`, `scan_completed`

## Component roles

| Stage | Crate/Module | Persistence |
|-------|--------------|-------------|
| Discovery | `aisec-discovery` + `discovery_run` | `endpoints`, `scans` |
| Fingerprint | `aisec-fingerprint` + `fingerprint_service` | `endpoints.fingerprint_json` |
| Planner | `aisec-planner` (wizard preview) | `playbook_json` categories from UI |
| Generator | `aisec-generator` | Pre-generated payloads in memory per job |
| Harness | `aisec-harness` + `plugin_transport` | — |
| Judge | `aisec-judge` + plugins | `findings` |
| Report | `aisec-report` | `reports` + files in `reports/` |

## Control

| Feature | Status | Evidence |
|---------|--------|----------|
| Pause/resume/stop | ✅ | `scan_pause/resume/stop` |
| Progress events | ✅ | `scan-progress` |
| Progress in DB | ✅ | `playbook_json.progress` |
| Parallelism | ⚠️ Sequential nested loops | `scan.rs:194-280` |
| Queue/scheduler | ❌ | Single job per scan id in memory |
| Cancellation | ✅ | `AtomicBool` cancel flag |
| Resume after app restart | ❌ | Jobs in-memory only; DB status may be stale |

---

# 10. Backend Audit

## Tauri commands: **90 total** (`lib.rs:169-259`)

See **Appendix A** for full list.

## AppState (`state.rs:17-32`)

| Field | Type |
|-------|------|
| `db` | SQLite `Database` |
| `data_dir` | `PathBuf` |
| `jobs` | `ScanJobManager` |
| `auth_engine_config` | `AuthEngineConfig` |
| `harness_factory` | `HarnessFactory` |
| `plugin_manager` | `Arc<Mutex<PluginManager>>` |
| `model_manager` | `Arc<Mutex<LocalModelManager>>` |
| `model_provider` | `SharedModelProvider` |
| `model_catalog_meta` | `BuiltinCatalogMeta` |
| `runtime_manager` | `Arc<Mutex<RuntimeManager>>` |
| `runtime_config_cache` | `Arc<Mutex<Option<RuntimeConfigurationDto>>>` |
| `runtime_model_loading_id` | `Arc<Mutex<Option<String>>>` |

**Singletons:** One `AppState` per process via `app.manage` (`lib.rs:147-158`). One `RuntimeManager` inside. `runtime_watch` uses static `WATCH_STARTED` (`runtime_watch.rs:14-18`).

## SQLite

- **File:** `<data_dir>/aisec.db` (`db.rs:17,24`)
- **Migrations:** `crates/aisec-storage/migrations/001-006`
- **Tables:** projects, targets, scans, findings, findings_fts, payloads, attack_results, reports, models, plugins, auth_*, endpoints

## JSON persistence

See **Appendix E**.

## src-tauri modules

`commands`, `db`, `dto`, `error`, `events`, `fingerprint_service`, `jobs`, `logging`, `method_heuristic`, `harness_runtime`, `judge_config`, `model_registry`, `third_party_credentials`, `embedded_runtime`, `plugin_service`, `plugin_transport`, `playwright_runtime`, `agent_service`, `ai_inference_settings`, `planner_service`, `generator_service`, `runtime_watch`, `session_auth`, `state`

---

# 11. Architecture Audit

## Layers (actual)

| Layer | Implementation |
|-------|----------------|
| **UI** | React 18 + Vite `src/` |
| **Application** | Tauri IPC commands, `AppStore`, page-level hooks |
| **Domain** | `crates/aisec-*` (judge, attack, discovery, etc.) |
| **Infrastructure** | `aisec-storage`, `aisec-auth`, file JSON configs |
| **Runtime** | `aisec-runtime` + llama-server subprocess |
| **Harness** | `aisec-harness` HTTP/Playwright |
| **Judge** | `aisec-judge` |
| **Storage** | SQLite + JSON files |
| **Plugin** | `aisec-plugin-host` subprocess sandbox |
| **Security** | `security_audit`, `aisec-auth` secrets, credential vaults |

## Violations / duplication

1. **Two AI config sources** — UI runtime vs scan inference
2. **Judge crate depends on Harness** — for `NormalizedResponse` type only
3. **Tauri commands orchestrate domain** — no application service layer
4. **`runtime` ↔ `models` command import cycle**
5. **Generator crate depends on Planner crate** — compile-time coupling

## Circular dependencies

- **Crates:** None found (DAG toward `aisec-core`)
- **Tauri modules:** `runtime.rs` ↔ `models.rs` mutual imports

---

# 12. Feature Matrix (selected — full matrix in Appendix F)

| Feature | Action/UI | Status | Entry | Source | IPC |
|---------|-----------|--------|-------|--------|-----|
| Create project | New Project modal | ✅ | Projects, Dashboard | `ProjectsPage`, `NewProjectModal` | `project_create` |
| Delete project | Actions dropdown | ✅ | Project details | `ProjectDetailsPage` | `project_delete` |
| Run discovery | Discovery button | ✅ | Discovery, Wizard | `DiscoveryPage`, `DiscoveryStep` | `discovery_run` |
| Start scan | Start Scan | ✅ | Wizard step 5 | `ScanWizardPage` | `scan_start` |
| Pause scan | Pause button | ✅ | Scans monitor | `ScansPage` | `scan_pause` |
| Load local model | Load in runtime | ✅ | AI Runtime | `AIRuntimePage` | `runtime_load_model` |
| Switch AI route | Mode toggle | ✅ | AI Runtime | `AIRuntimePage` | `runtime_set_inference_route` |
| Agent LLM planner | — | ❌ | Agent scan | `agent_service.rs:96-99` | — |
| Stream inference | — | ❌ | — | `llama_cpp_runtime.rs:229` | — |
| Plugin report hook | — | ❌ | — | No invoke in report path | — |
| Jailbreak attack category | Disabled "(soon)" | ❌ UI | Attacks page | `AttacksPage.tsx:18-23` | `attack_run` supports categories in engine |

---

# 13. Action Matrix

## Projects

| Action | UI | Backend | IPC | Persistence | Status |
|--------|-----|---------|-----|-------------|--------|
| Create | NewProjectModal | `project_create` | ✅ | SQLite `projects` | ✅ |
| List | ProjectsPage | `project_list` | ✅ | SQLite | ✅ |
| Get | ProjectDetails | `project_get` | ✅ | SQLite | ✅ |
| Update | EditProjectModal | `project_update` | ✅ | SQLite | ✅ |
| Delete | ActionsDropdown | `project_delete` | ✅ | SQLite | ✅ |
| Archive | — | — | — | — | ❌ |
| Import/Export | — | — | — | — | ❌ |
| Duplicate | — | — | — | — | ❌ |
| New Scan | Link | — | — | navigates | ✅ |

## Targets

| Action | UI | IPC | Persistence | Status |
|--------|-----|-----|-------------|--------|
| Create | AddTargetModal, Wizard | `target_create` | `targets` | ✅ |
| List | TargetsPage | `target_list` | SQLite | ✅ |
| Get | TargetDetails | `target_get` | SQLite | ✅ |
| Update | — | `endpoint_update` for endpoints only | — | ⚠️ |
| Delete | — | — | — | ❌ |

## Scans

| Action | UI | IPC | Status |
|--------|-----|-----|--------|
| Create (wizard) | ScanWizard | `scan_start` | ✅ |
| List | ScansPage, store | `scan_list` | ✅ |
| Get | ScanDetails | `scan_get` | ✅ |
| Pause/Resume/Stop | ScanMonitorCard | `scan_pause/resume/stop` | ✅ |
| Status poll | Monitor | `scan_status` | ✅ |

## Discovery

| Action | UI | IPC | Status |
|--------|-----|-----|--------|
| Run | DiscoveryPage, Wizard | `discovery_run` | ✅ |
| List endpoints | Discovery details | `endpoint_list` | ✅ |
| Create endpoint manual | Wizard | `endpoint_create` | ✅ |
| Update endpoint | Wizard | `endpoint_update` | ✅ |

## Runtime

| Action | UI | IPC | Status |
|--------|-----|-----|--------|
| Install | AI Runtime | `runtime_install` | ✅ |
| Start/Stop/Restart | AI Runtime | `runtime_start/stop/restart` | ✅ |
| Load/Unload model | AI Runtime | `runtime_load/unload_model` | ✅ |
| Set route | Mode toggle | `runtime_set_inference_route` | ✅ |
| Health/Benchmark | AI Runtime | `runtime_health/benchmark` | ✅ |

## Models

| Action | UI | IPC | Status |
|--------|-----|-----|--------|
| Download catalog | ModelsPage | `models_download_start` | ✅ |
| Import GGUF/ZIP | Add modal | `models_import_*` | ✅ |
| Add third-party | Add modal | `models_save_third_party` | ✅ |
| Test | Registry actions | `models_test_*` | ✅ |
| Remove | Registry | `models_remove` | ✅ |

## Reports

| Action | UI | IPC | Status |
|--------|-----|-----|--------|
| Generate | Reports, Results | `report_generate` | ✅ |
| Export file | Reports | `report_export` | ✅ |
| List | ReportsPage | `report_list_all` | ✅ |

## Settings

| Action | UI | IPC | Status |
|--------|-----|-----|--------|
| Security audit | Settings | `security_audit` | ✅ |
| Migrate secrets | Settings | `security_migrate_secrets` | ✅ |
| Registry diagnostics | Settings | `models_registry_diagnostics` | ✅ |
| Theme/offline toggles | Settings | Store only | ✅ client |

---

# 14. State Management Audit

| State | Technology | Location | Persistence |
|-------|------------|----------|-------------|
| Workspace data | React Context + `useReducer` | `AppStore.tsx` | Refreshed from SQLite via IPC |
| Toast | React Context | `ToastProvider.tsx` | Transient |
| Wizard draft | `useState` + localStorage | `wizardState.ts` | `aisec:scan-wizard` |
| View mode / page size | Custom hooks | `useViewPreference`, `usePageSizePreference` | localStorage |
| AI Runtime config | Hook + IPC | `useAiInferenceRoute` | Backend JSON |
| Model loading overlay | Global hook + poller | `useRuntimeModelLoading` | Backend `runtime_model_loading_id` |
| Scan job progress | Backend mutex + events | `ScanJobManager`, `scan-progress` | Partial in `playbook_json` |
| Auth recording | Managed mutex | `AuthRecordingState` `lib.rs:261` | Transient |
| Settings UI prefs | AppStore reducer | `settings` in `AppStore.tsx:89-96` | In-memory only (lost on reload unless extended) |
| Zustand/Redux | — | **NOT FOUND** | — |
| Session Storage | — | **NOT FOUND** | — |

**Synchronization:** `app-data-changed` event → `AppStore.refresh()` (`AppStore.tsx:334-338`)

---

# 15. Event Flow Audit

| Event | Origin | IPC/Backend | DB | UI update |
|-------|--------|-------------|-----|-----------|
| **App startup** | `lib.rs setup` | bootstrap DB, runtime, models | migrations | `health` → Connected |
| **Open project** | Navigate | `project_get` optional | read | ProjectDetails |
| **Create scan** | Wizard Start | `scan_start` | insert scan | SubmitStep console |
| **Discovery** | Button | `discovery_run` | endpoints, scan | DiscoveryStep table |
| **Attack unit** | Scan job | harness + judge | findings | `scan-progress` events |
| **Judge verdict** | `attack.rs` | `JudgeEngine::judge_normalized` | findings insert | progress event |
| **Report** | Export btn | `report_generate` | reports + file | toast + refresh |
| **Install model** | Models UI | `models_download_*` | registry.json | DownloadManagerCard poll |
| **Runtime start** | AI Runtime | `runtime_start` | manifest | status cards refresh |
| **Runtime stop** | AI Runtime / exit | `runtime_stop` / shutdown | — | status |
| **Auth record** | Playwright panel | `auth_record_session_*` | auth_sessions | target descriptor |

---

# 16. Dependency Graph

See also `docs/AI_RUNTIME_ARCHITECTURE_AUDIT.md` Section 21 for detailed mermaid diagrams.

## Frontend (logical)

```
Pages → shared/ipc/*.ts → Tauri invoke → commands/* → AppState → crates/*
```

## Backend crates (compile-time)

```
aisec-desktop → all feature crates
aisec-generator → aisec-planner → aisec-fingerprint → aisec-core
aisec-judge → aisec-harness, aisec-runtime, aisec-models
aisec-discovery → aisec-core (isolated)
aisec-report → aisec-storage
```

## Runtime communication

```
RuntimeManager → RuntimeSupervisor → llama-server (subprocess)
                → reqwest HTTP → /completion, /health
```

**NOT FOUND:** Named pipe, Unix socket for inference.

---

# 17. Technical Debt

| Severity | Item | Why | Impact | Direction |
|----------|------|-----|--------|-----------|
| **Critical** | Dual AI config files | Independent persistence | Wrong model used in scans vs UI | Unify read path |
| **Critical** | Agent LocalLlm not wired | `None` generator backend | Agent mode fails on LLM retry | Wire `build_generator_llm_backend` |
| **High** | Triplicated LLM backend build | Copy-paste in 3 modules | Drift, bugs | Extract shared module |
| **High** | Judge Provider vs AI Runtime UX | Two surfaces | User confusion | Consolidate UI |
| **High** | `runtime`↔`models` import cycle | Mutual command imports | Hard to refactor | Extract shared service module |
| **Medium** | Unused IPC commands | Dead API surface | Maintenance noise | Remove or wire UI |
| **Medium** | Legacy Ollama types/defaults | Post-migration leftovers | Misconfiguration | Deprecate paths |
| **Medium** | Discovery single worker | Crawler deadlock | Slow discovery | Fix crawler concurrency |
| **Medium** | Findings `scanId` URL not read | Incomplete deep link | UX bug | Parse searchParams |
| **Low** | Dashboard no runtime refresh | Load once | Stale card | Poll or event listen |
| **Low** | Hardcoded ROCm display | Placeholder | Inaccurate UI | Wire hardware detect |
| **Low** | Stale docs in `docs/` | Pre-implementation notes | Onboarding friction | Update docs |

---

# 18. Missing Features (from codebase intentions only)

Evidence = disabled UI, unused IPC, comments, or struct fields not wired — **not** external product comparisons.

| Intended (in code) | Evidence | Status |
|--------------------|----------|--------|
| Attack categories beyond prompt_injection | `AttacksPage.tsx:18-23` "(soon)" | ❌ UI disabled |
| Agent `planner_mode` LocalLlm | `AgentConfig.planner_mode` exists; always `Deterministic` `agent_service.rs:207` | ❌ not wired |
| Agent generator LocalLlm on retry | `retry.rs` escalates mode; `generate_from_plan(..., None)` | ❌ broken |
| Plugin `render_report` hook | `PluginType::Report` without invoke in `report_generate_op` | ❌ not wired |
| `models_test_embeddings` in UI | IPC registered `lib.rs:231` | ❌ no UI caller found |
| `models_install` in UI | IPC exists; UI uses `models_download_start` | ⚠️ legacy IPC unused by UI |
| `getRuntimeStatus` / `getRuntimeInferenceSettings` | `runtime.ts` exports | ❌ no feature usage |
| Multi-worker discovery | Comment pins `worker_count: 1` `discovery.rs:220-223` | ❌ blocked by deadlock |
| LLM streaming | `"stream": false` in runtime clients | ❌ explicitly off |
| Findings filter by URL `scanId` | Links from ResultsStep | ❌ not implemented on FindingsPage |

---

# 19. Architecture Score

| Dimension | Score /100 | Notes |
|-----------|------------|-------|
| UI | 88 | Complete routes; minor deep-link gaps |
| Backend | 86 | 90 commands; solid SQLite |
| Runtime | 80 | Strong lifecycle; no inference gateway |
| Harness | 82 | HTTP + Playwright wired |
| Models | 85 | Full vault; split activation |
| Judge | 78 | Full engine; config split from runtime |
| AI (unified) | 38 | No AiService; fragmented |
| Plugin | 65 | Host works; report plugins not hooked |
| Security Pack | 75 | `aisec-payload` static; security audit IPC |
| Performance | 60 | Sequential scan loops; discovery single-worker |
| Scalability | 55 | Desktop single-process design |
| Maintainability | 50 | Duplication, dual config |
| Extensibility | 62 | Plugin hooks partial |
| **Overall** | **72** | Product usable; architecture consolidation needed |

---

# 20. Appendix

## Appendix A — All Tauri Commands (90)

`health`, `app_info`, `db_health`, `project_create`, `project_list`, `project_get`, `project_update`, `project_delete`, `target_create`, `target_list`, `target_get`, `scan_create`, `scan_list`, `scan_get`, `finding_list`, `finding_list_all`, `report_generate`, `report_list`, `report_list_all`, `report_read`, `report_export`, `discovery_run`, `endpoint_list`, `endpoint_create`, `endpoint_update`, `attack_run_prompt_injection`, `scan_start`, `scan_status`, `scan_pause`, `scan_resume`, `scan_stop`, `auth_record_session_start`, `auth_record_session_finish`, `auth_record_session_cancel`, `auth_session_validate`, `auth_session_status`, `judge_config_get`, `judge_config_save`, `judge_test_connectivity`, `judge_test_model`, `models_list`, `models_registry_info`, `models_registry_diagnostics`, `models_browse`, `models_install`, `models_import_gguf`, `models_save_third_party`, `models_third_party_edit_form`, `models_test_third_party`, `models_test_connection`, `models_import_zip`, `models_download_start`, `models_download_status`, `models_download_pause`, `models_download_resume`, `models_download_cancel`, `models_download_retry_verify`, `models_download_cancel_verify`, `models_remove`, `models_verify`, `models_test_inference`, `models_test_embeddings`, `models_vault_path`, `models_vault_stats`, `planner_generate`, `generator_generate`, `runtime_status`, `runtime_install`, `runtime_repair`, `runtime_start`, `runtime_stop`, `runtime_delete`, `runtime_load_model`, `runtime_unload_model`, `runtime_restart`, `runtime_health`, `runtime_benchmark`, `runtime_logs`, `runtime_hardware`, `hardware_refresh`, `runtime_configuration`, `runtime_inference_settings`, `runtime_set_inference_route`, `security_audit`, `security_migrate_secrets`, `plugins_list`, `plugins_refresh`, `plugins_enable`, `plugins_disable`, `plugins_info`

**Source:** `src-tauri/src/lib.rs:169-259`

## Appendix B — All Routes (17 + fallback)

`/`, `/projects`, `/projects/:projectId`, `/scans`, `/scans/new`, `/scans/:scanId`, `/targets`, `/targets/:targetId`, `/discovery`, `/discovery/:scanId`, `/attacks`, `/findings`, `/reports`, `/judge`, `/runtime`, `/models`, `/plugins`, `/settings`, `*` → `/`

## Appendix C — SQLite Tables

`projects`, `targets`, `scans`, `findings`, `findings_fts`, `payloads`, `attack_results`, `reports`, `models`, `plugins`, `auth_profiles`, `auth_sessions`, `auth_recordings`, `endpoints`, `_sqlx_migrations`

**Migrations:** `001_initial_schema.sql` through `006_auth_secure_credentials.sql`

## Appendix D — Workspace Crates (17)

`aisec-core`, `aisec-attack`, `aisec-payload`, `aisec-models`, `aisec-judge`, `aisec-report`, `aisec-fingerprint`, `aisec-planner`, `aisec-generator`, `aisec-agent`, `aisec-plugin-host`, `aisec-auth`, `aisec-discovery`, `aisec-harness`, `aisec-runtime`, `aisec-storage`, `aisec-desktop` (+ `aisec-integration-tests`)

## Appendix E — JSON / File Persistence

| Path | Purpose |
|------|---------|
| `aisec.db` | SQLite |
| `judge_config.json` | Judge provider config |
| `ai_inference_settings.json` | Runtime route + selected model |
| `plugins_state.json` | Plugin enablement |
| `runtime/manifest.json` | llama-server install |
| `runtime/hardware.json` | Hardware profile |
| `models/registry.json` | Model registry |
| `models/.credentials/*.enc` | Encrypted API keys |
| `reports/*` | Generated report files |
| `AuthSessions/*.storage.enc` | Browser session vault |

## Appendix F — React Contexts

1. `AppStoreContext` — `AppStore.tsx:157`
2. `ToastContext` — `ToastProvider.tsx:23`

**NOT FOUND:** Zustand stores, Redux, additional Contexts for runtime/models.

## Appendix G — Tauri Events

| Event | Constant | Evidence |
|-------|----------|----------|
| Scan progress | `scan-progress` | `events.rs:7` |
| Data changed | `app-data-changed` | `events.rs:8` |
| Runtime install | `runtime-install-progress` | `events.rs:9` |

## Appendix H — Judge Evaluators

| Evaluator | Module | Kind |
|-----------|--------|------|
| Rule-based | `evaluators/rule.rs` | `EvaluatorKind::Rule` |
| Regex | `evaluators/regex.rs` | `EvaluatorKind::Regex` |
| LLM | `evaluators/llm.rs` | `EvaluatorKind::Llm` |

**Roles:** Judge, Classifier, Attacker — `types.rs:7-14`

## Appendix I — Harness Types

`HttpHarness`, `OpenAiHarness`, `PlaywrightHarness` (conditional) — `harness_factory.rs:19-27`, `harness_runtime.rs:80-96`

## Appendix J — Attack Categories (engine)

Built-in categories in `aisec-attack` — `attacks/mod.rs:27-38` (includes prompt_injection, jailbreak, etc.)

## Appendix K — Frontend IPC Functions (`src/shared/ipc/`)

| Module | Function | Backend command (typical) |
|--------|----------|---------------------------|
| `client.ts` | `healthCheck` | `health` |
| `client.ts` | `getAppInfo` | `app_info` |
| `projects.ts` | `listProjects`, `createProject`, etc. | `project_*` |
| `domain.ts` | `listTargets`, `createTarget`, `listScans`, `startScan`, `getScan`, `listFindings*`, `generateReport`, `report_*` | `domain::*` |
| `discovery.ts` | `runDiscovery`, `listEndpoints`, `createEndpoint`, `updateEndpoint` | `discovery::*` |
| `attacks.ts` | `runPromptInjection` | `attack_run_prompt_injection` |
| `scan.ts` | `getScanStatus`, `pauseScan`, `resumeScan`, `stopScan` | `scan_*` |
| `auth.ts` | `start/finish/cancelAuthRecordSession`, `validateAuthSession`, `fetchAuthSessionStatus` | `auth_*` |
| `judge.ts` | `get/saveJudgeConfig`, `testJudgeConnectivity`, `testJudgeModel` | `judge_*` |
| `models.ts` | 20+ model vault functions | `models_*` |
| `runtime.ts` | 17 runtime functions | `runtime_*`, `hardware_refresh` |
| `planner.ts` | `generateAttackPlan` | `planner_generate` |
| `generator.ts` | `generatePromptPayloads` | `generator_generate` |
| `plugins.ts` | `list/refresh/enable/disablePlugins`, `getPluginsInfo` | `plugins_*` |
| `security.ts` | `securityAudit`, `securityMigrateSecrets` | `security_*` |
| `dialog.ts` | `pickModelImportFile`, `pickAnyModelImportFile` | Tauri dialog plugin |

**Invoke wrapper:** `invokeCommand` in `src/shared/ipc/invoke.ts:16`

## Appendix L — Shared Non-Page Components (feature-level)

| Component | File | Consumers |
|-----------|------|-----------|
| `NewProjectModal` | `projects/NewProjectModal.tsx` | ProjectsPage |
| `EditProjectModal` | `projects/EditProjectModal.tsx` | ProjectDetailsPage |
| `AddTargetModal` | `targets/AddTargetModal.tsx` | TargetsPage |
| `AddModelModal` | `models/AddModelModal.tsx` | ModelsPage |
| `ModelRegistrySection` | `models/ModelRegistrySection.tsx` | ModelsPage |
| `DownloadManagerCard` | `models/DownloadManagerCard.tsx` | ModelsPage |
| `HuggingFaceModelCatalog` | `models/HuggingFaceModelCatalog.tsx` | AddModelModal |
| `ThirdPartyModelsPanel` | `models/ThirdPartyModelsPanel.tsx` | AddModelModal |
| `AiRuntimeDashboardCard` | `dashboard/AiRuntimeDashboardCard.tsx` | DashboardPage |
| `ScanMonitorCard` | `scans/ScanMonitorCard.tsx` | ScansPage |
| `ScanHistoryCard` | `scans/ScanHistoryCard.tsx` | ScansPage |
| `WizardStepper` | `scans/WizardStepper.tsx` | ScanWizardPage |
| `ScanConsole` | `scans/ScanConsole.tsx` | SubmitStep |
| `TargetFormFields` | `scans/TargetFormFields.tsx` | TargetStep |
| `PlaywrightRecordPanel` | `scans/PlaywrightRecordPanel.tsx` | TargetStep |
| `RegistryDiagnosticsPanel` | `settings/RegistryDiagnosticsPanel.tsx` | SettingsPage |
| `RuntimeModelLoadingPoller` | `app/providers/RuntimeModelLoadingPoller.tsx` | AppProviders |

## Appendix M — Context Menus

**Status:** ❌ NOT IMPLEMENTED — I could not find `contextmenu`, `onContextMenu`, or right-click menu handlers in `src/features/` (grep returned no dedicated context menu pattern).

## Appendix N — Screenshots

**Status:** ❌ NOT AVAILABLE — No screenshot assets or visual regression captures found in repository for audit inclusion.

## Appendix O — Expanded Feature Matrix (UI actions → backend)

| # | Domain | UI element | Route/Dialog | IPC | DB/File | Status |
|---|--------|------------|--------------|-----|---------|--------|
| 1 | Project | New Project button | `/projects` modal | `project_create` | `projects` | ✅ |
| 2 | Project | Delete | Project details dropdown | `project_delete` | `projects` | ✅ |
| 3 | Project | Edit | EditProjectModal | `project_update` | `projects` | ✅ |
| 4 | Project | Refresh | ProjectsPage header | `project_list` (via refresh) | — | ✅ |
| 5 | Target | Add Target | AddTargetModal | `target_create` | `targets` | ✅ |
| 6 | Target | New Scan link | Target details | navigate | — | ✅ |
| 7 | Scan | New Scan | Scans header | navigate wizard | — | ✅ |
| 8 | Scan | Pause | ScanMonitorCard | `scan_pause` | `scans.status` | ✅ |
| 9 | Scan | Resume | ScanMonitorCard | `scan_resume` | `scans.status` | ✅ |
| 10 | Scan | Stop | ScanMonitorCard | `scan_stop` | `scans.status` | ✅ |
| 11 | Wizard | Start Scan | Step 5 footer | `scan_start` | `scans` + playbook | ✅ |
| 12 | Wizard | Run discovery | Step 3 | `discovery_run` | endpoints | ✅ |
| 13 | Wizard | Generate plan | Step 4 | `planner_generate` | — (preview) | ✅ |
| 14 | Wizard | Generate payloads | Step 4 | `generator_generate` | — (preview) | ✅ |
| 15 | Wizard | Export report | Step 6 | `report_generate` + `report_export` | reports + file | ✅ |
| 16 | Discovery | Run discovery | Discovery table/tree | `discovery_run` | endpoints | ✅ |
| 17 | Attack | Launch Attack | Attacks page | `attack_run_prompt_injection` | findings | ✅ |
| 18 | Attack | Jailbreak category | Attacks select | — | — | ❌ disabled UI |
| 19 | Finding | Filter severity | Findings chips | client filter | — | ✅ |
| 20 | Finding | Filter by scan URL | — | — | — | ❌ param ignored |
| 21 | Report | Export HTML/PDF/SARIF | Reports page | `report_generate` | reports/ | ✅ |
| 22 | Judge | Save config | Judge page | `judge_config_save` | judge_config.json | ✅ |
| 23 | Judge | Test connectivity | Judge page | `judge_test_connectivity` | — | ✅ |
| 24 | Runtime | Install | AI Runtime | `runtime_install` | manifest | ✅ |
| 25 | Runtime | Load model | AI Runtime local | `runtime_load_model` | llama-server | ✅ |
| 26 | Runtime | Mode toggle | AI Runtime | `runtime_set_inference_route` | ai_inference_settings.json | ✅ |
| 27 | Runtime | Third-party Use | Model row | `runtime_set_inference_route` + test | settings + registry | ✅ |
| 28 | Models | Download catalog | Models page | `models_download_start` | registry + files | ✅ |
| 29 | Models | Import GGUF | Add modal | `models_import_gguf` | registry | ✅ |
| 30 | Models | Remove | Registry dropdown | `models_remove` | registry | ✅ |
| 31 | Models | Test inference | Registry | `models_test_inference` | — | ✅ |
| 32 | Models | Test embeddings | — | `models_test_embeddings` | — | ❌ no UI |
| 33 | Plugins | Enable/Disable | Plugins table | `plugins_enable/disable` | plugins_state.json | ✅ |
| 34 | Settings | Security migrate | Settings security tab | `security_migrate_secrets` | keychain | ✅ |
| 35 | Settings | Registry diagnostics | Settings troubleshooting | `models_registry_diagnostics` | — | ✅ |
| 36 | Auth | Record session | Wizard target step | `auth_record_session_*` | auth_sessions | ✅ |
| 37 | Dashboard | Open AI Runtime | Runtime card click | `runtime_configuration` | — | ✅ |
| 38 | Agent | Local LLM planner | Agent scan | — | — | ❌ hardcoded deterministic |
| 39 | Agent | Local LLM generator retry | Agent scan | — | — | ❌ None backend |
| 40 | Plugin | Report render | — | — | — | ❌ not invoked |

---

*End of master audit. No application source code was modified. Evidence gathered from repository read-only analysis.*
