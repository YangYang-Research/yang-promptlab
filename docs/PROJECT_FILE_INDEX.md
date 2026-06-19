# AISec — Project File Index

> Index of important project files with path, purpose, dependencies, complexity, and status.  
> Generated: 2026-06-13 · Version: 0.1.0

**Complexity:** Low = simple/config; Medium = moderate logic; High = orchestration/core engine  
**Status:** Complete · Partial · Stub · Test-only · Docs · Generated · Unregistered

---

## Frontend

### Entry & shell

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `index.html` | Vite HTML shell; mounts React root | Vite | Low | Complete |
| `src/main.tsx` | React DOM entry point | `App.tsx` | Low | Complete |
| `src/App.tsx` | Backend health bootstrap; gates render on IPC probe | `@/shared/ipc`, `AppStore` | Medium | Complete |
| `src/vite-env.d.ts` | Vite/TS ambient types | — | Low | Complete |
| `src/app/providers/AppProviders.tsx` | Wraps store, toast, error boundary | `AppStore`, `ToastProvider`, `ErrorBoundary` | Low | Complete |
| `src/app/layout/MainLayout.tsx` | Page chrome: sidebar + top bar + outlet | `Sidebar`, `TopBar`, React Router | Low | Complete |
| `src/app/layout/Sidebar.tsx` | Primary navigation from `nav.ts` | `nav.ts`, React Router | Medium | Complete |
| `src/app/layout/TopBar.tsx` | Search, backend connection indicator | `AppStore`, `SearchInput` | Medium | Complete |
| `src/app/layout/NavIcon.tsx` | SVG icons for sidebar items | — | Low | Complete |
| `src/app/router/AppRouter.tsx` | Hash routes + lazy-loaded pages | React Router, all feature pages | Medium | Complete |
| `src/app/router/nav.ts` | Nav item definitions (paths, icons, sections) | — | Low | Complete |

### Global state

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `src/app/store/AppStore.tsx` | Workspace data load, mutations, IPC refresh | `@/shared/ipc`, mappers, stats | High | Complete |
| `src/app/store/types.ts` | Store state, actions, domain view types | `@/shared/types` | Medium | Complete |
| `src/app/store/mappers.ts` | DTO → UI model mapping (projects, scans, findings) | IPC DTO types | Medium | Complete |

### Shared — IPC client

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `src/shared/ipc/invoke.ts` | Tauri `invoke` wrapper + error normalization | `@tauri-apps/api` | Medium | Complete |
| `src/shared/ipc/client.ts` | Domain command wrappers + DTO type exports | `invoke.ts` | High | Complete |
| `src/shared/ipc/projects.ts` | Project CRUD IPC (`project_*`) | `invoke.ts` | Low | Complete |
| `src/shared/ipc/auth.ts` | Playwright auth recording IPC | `invoke.ts` | Medium | Complete |
| `src/shared/ipc/index.ts` | Re-exports all IPC modules | client, projects, auth | Low | Complete |

### Shared — types, errors, logging, stats

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `src/shared/types/index.ts` | Frontend domain types (Project, Target, Finding, …) | — | Medium | Complete |
| `src/shared/errors/AppError.ts` | Normalized app error type + `toAppError()` | — | Medium | Complete |
| `src/shared/errors/ErrorBoundary.tsx` | React error boundary UI | `AppError` | Medium | Complete |
| `src/shared/errors/index.ts` | Error module re-exports | — | Low | Complete |
| `src/shared/logging/logger.ts` | Structured frontend logger | — | Low | Complete |
| `src/shared/logging/index.ts` | Logger re-export | — | Low | Complete |
| `src/shared/stats.ts` | Dashboard stat aggregation, severity counts | domain types | Medium | Complete |
| `src/shared/targetScanContext.ts` | Helpers linking targets to scans in UI | domain types | Low | Complete |
| `src/shared/notifications/ToastProvider.tsx` | Toast notification context | React | Medium | Complete |
| `src/shared/notifications/index.ts` | Toast hook re-export | — | Low | Complete |

### Shared — hooks & utils

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `src/shared/hooks/usePaginatedList.ts` | Client-side pagination state | — | Low | Complete |
| `src/shared/hooks/usePageSizePreference.ts` | Persist page size in sessionStorage | — | Low | Complete |
| `src/shared/hooks/useViewPreference.ts` | List/card view mode preference | — | Low | Complete |
| `src/shared/utils/pagination.ts` | Pagination slice helpers | — | Low | Complete |

### Shared — UI components

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `src/shared/components/index.ts` | Component barrel exports | all components | Low | Complete |
| `src/shared/components/Button.tsx` | Primary/ghost button variants | — | Low | Complete |
| `src/shared/components/Card.tsx` | Card + StatCard layout | — | Low | Complete |
| `src/shared/components/Badge.tsx` | Badge, SeverityBadge, StatusBadge | — | Medium | Complete |
| `src/shared/components/DataTable.tsx` | Sortable/selectable data table | — | High | Complete |
| `src/shared/components/EmptyState.tsx` | Empty list placeholder | — | Low | Complete |
| `src/shared/components/Modal.tsx` | Modal dialog shell | — | Medium | Complete |
| `src/shared/components/PageHeader.tsx` | Page title + actions row | — | Low | Complete |
| `src/shared/components/ProgressBar.tsx` | Progress indicator | — | Low | Complete |
| `src/shared/components/SearchInput.tsx` | Search field | — | Low | Complete |
| `src/shared/components/Select.tsx` | Styled select input | — | Low | Complete |
| `src/shared/components/Pagination.tsx` | Pagination + PageSizeSelect + ContentToolbar | — | Medium | Complete |
| `src/shared/components/ActionsDropdown.tsx` | Row action menu | — | Medium | Complete |
| `src/shared/components/IconButton.tsx` | Icon-only button | Icons | Low | Complete |
| `src/shared/components/Icons.tsx` | Shared SVG icons | — | Low | Complete |
| `src/shared/components/RefreshButton.tsx` | Refresh with loading state | Button, Icons | Low | Complete |
| `src/shared/components/ListCard.tsx` | Card-based list item layout | Card, Badge | Medium | Complete |
| `src/shared/components/ViewModeToggle.tsx` | Table/card view toggle | Icons | Low | Complete |

### Shared — styles

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `src/styles/global.css` | Design tokens, layout, page/feature styles | — | High | Complete |

### Feature — dashboard

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `src/features/dashboard/DashboardPage.tsx` | Overview: stats, severity chart, activity, jobs | `AppStore`, shared components | Medium | Partial (hardcoded hints; empty activity/jobs) |

### Feature — projects

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `src/features/projects/ProjectsPage.tsx` | Project list, create/delete | `AppStore`, `NewProjectModal` | Medium | Complete |
| `src/features/projects/ProjectDetailsPage.tsx` | Single project: targets, scans, actions | `AppStore`, IPC | Medium | Complete |
| `src/features/projects/NewProjectModal.tsx` | Create project form modal | `AppStore.actions` | Low | Complete |
| `src/features/projects/EditProjectModal.tsx` | Edit project name/description | `updateProject` IPC | Low | Complete |

### Feature — targets

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `src/features/targets/TargetsPage.tsx` | Target list across projects | `AppStore` | Medium | Complete |
| `src/features/targets/TargetDetailsPage.tsx` | Target descriptor detail view | `AppStore`, `getTarget` | Medium | Complete |
| `src/features/targets/AddTargetModal.tsx` | Quick target create (non-wizard) | `targetDescriptor`, IPC | Medium | Complete |

### Feature — scans & wizard

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `src/features/scans/ScanWizardPage.tsx` | 6-step scan wizard orchestrator | all steps, `wizardState`, IPC | High | Complete |
| `src/features/scans/ScansPage.tsx` | Scan history list | `AppStore` | Medium | Complete |
| `src/features/scans/ScanDetailsPage.tsx` | Scan monitor, playbook, findings link | `useScanStatuses`, IPC | High | Complete |
| `src/features/scans/ScanMonitorCard.tsx` | Live scan progress card | `useScanStatuses` | Medium | Complete |
| `src/features/scans/WizardStepper.tsx` | Step indicator UI | `wizardSteps` | Low | Complete |
| `src/features/scans/wizardSteps.ts` | Step defs, navigation/completion rules | `targetDescriptor`, types | Medium | Complete |
| `src/features/scans/wizardState.ts` | Wizard session schema + sessionStorage I/O | `attackProfiles`, steps | High | Complete |
| `src/features/scans/targetDescriptor.ts` | Target form schema, auth kinds, validation, descriptor JSON | — | High | Complete |
| `src/features/scans/TargetFormFields.tsx` | Target URL + auth method UI | `PlaywrightRecordPanel` | High | Complete |
| `src/features/scans/PlaywrightRecordPanel.tsx` | Interactive login recording UI | `auth` IPC | Medium | Complete |
| `src/features/scans/attackProfiles.ts` | Attack profile + category definitions | — | Medium | Complete |
| `src/features/scans/discoveryPhases.ts` | Discovery phase labels for step animation | — | Low | Complete |
| `src/features/scans/scanPlaybook.ts` | Playbook JSON parsing helpers | — | Low | Complete |
| `src/features/scans/scanDetailsHelpers.ts` | Auth type labels, scan summary formatting | — | Low | Complete |
| `src/features/scans/useScanStatuses.ts` | Poll `scan_status` for running scans | IPC | Medium | Complete |
| `src/features/scans/steps/ProjectStep.tsx` | Wizard step 1: project selection | `AppStore` | Low | Complete |
| `src/features/scans/steps/TargetStep.tsx` | Wizard step 2: target + auth persist | `targetDescriptor`, IPC | Medium | Complete |
| `src/features/scans/steps/DiscoveryStep.tsx` | Wizard step 3: run discovery, select endpoints | `discovery_run`, `endpoint_*` | High | Complete |
| `src/features/scans/steps/AttackPlanStep.tsx` | Wizard step 4: profile + categories | `attackProfiles` | Medium | Complete |
| `src/features/scans/steps/SubmitStep.tsx` | Wizard step 5: review + `scan_start` | IPC | Medium | Complete |
| `src/features/scans/steps/ResultsStep.tsx` | Wizard step 6: findings + report export | `AppStore`, reports | Medium | Complete |

### Feature — discovery, attacks, findings, reports, models, settings

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `src/features/discovery/DiscoveryPage.tsx` | Run discovery on saved target | `runDiscovery` IPC | Medium | Complete |
| `src/features/discovery/DiscoveryDetailsPage.tsx` | Endpoint list for a discovery scan | `endpoint_list` | Medium | Complete |
| `src/features/attacks/AttacksPage.tsx` | Ad-hoc prompt injection on endpoint | `attack_run_prompt_injection` | Medium | Complete |
| `src/features/findings/FindingsPage.tsx` | Global findings table + filters | `findingsFilters`, `AppStore` | High | Complete |
| `src/features/findings/findingsFilters.ts` | Finding filter/sort helpers | domain types | Medium | Complete |
| `src/features/reports/ReportsPage.tsx` | Report list + generate | `AppStore`, modal | Medium | Complete |
| `src/features/reports/GenerateReportModal.tsx` | Report format/kind picker | IPC | Medium | Complete |
| `src/features/reports/reportDownloads.ts` | Generate + export report helpers | IPC | Medium | Complete |
| `src/features/models/ModelsPage.tsx` | Local GGUF model management UI | `AppStore.models` | Medium | Stub (no IPC; empty data) |
| `src/features/settings/SettingsPage.tsx` | Theme, paths, backend status display | `AppStore.settings` | Medium | Partial (not persisted) |

---

## Backend

### Tauri shell — core

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `src-tauri/Cargo.toml` | Desktop crate manifest | workspace crates | Low | Complete |
| `src-tauri/tauri.conf.json` | Tauri app config, bundle, resources, hooks | — | Medium | Complete |
| `src-tauri/build.rs` | Tauri build script | tauri-build | Low | Complete |
| `src-tauri/capabilities/default.json` | Tauri 2 capability permissions | — | Low | Complete |
| `src-tauri/src/main.rs` | Binary entry; calls `lib::run()` | `aisec_desktop_lib` | Low | Complete |
| `src-tauri/src/lib.rs` | App setup: DB, logging, invoke_handler, shutdown | all commands, `AppState` | High | Complete |
| `src-tauri/src/state.rs` | Shared `AppState` (DB, repos, auth config, jobs) | aisec-storage, auth config | Medium | Complete |
| `src-tauri/src/db.rs` | DB path resolution + pool open/migrate | aisec-storage | Medium | Complete |
| `src-tauri/src/dto.rs` | IPC response DTOs (Project, Scan, Finding, …) | aisec-storage models | High | Complete |
| `src-tauri/src/error.rs` | `CommandError` envelope for IPC | aisec-core | Medium | Complete |
| `src-tauri/src/logging.rs` | tracing init for desktop | aisec-core | Low | Complete |
| `src-tauri/src/playwright_runtime.rs` | Resolve bundled vs dev Playwright paths | Tauri paths | Medium | Complete |
| `src-tauri/resources/playwright/.gitkeep` | Placeholder for bundled Playwright dir | — | Low | Complete |

### Tauri — IPC commands

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `src-tauri/src/commands/mod.rs` | `health`, `app_info`, `db_health` | AppState | Low | Partial (`db_health` unregistered) |
| `src-tauri/src/commands/projects.rs` | Project CRUD IPC | aisec-storage | Medium | Complete |
| `src-tauri/src/commands/domain.rs` | Target/scan/finding/report IPC | storage, aisec-report | High | Complete |
| `src-tauri/src/commands/discovery.rs` | `discovery_run`, `endpoint_list/create` | aisec-discovery, storage | High | Complete |
| `src-tauri/src/commands/attack.rs` | `attack_run_prompt_injection`, `run_category_on_endpoint` | aisec-attack, aisec-judge, storage | High | Complete |
| `src-tauri/src/commands/scan.rs` | `scan_start/status/pause/resume/stop` background jobs | attack, jobs, storage | High | Complete |
| `src-tauri/src/commands/auth.rs` | Playwright session record start/finish | aisec-auth | High | Complete |

### Tauri — background jobs

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `src-tauri/src/jobs/mod.rs` | Jobs module root | manager | Low | Complete |
| `src-tauri/src/jobs/manager.rs` | `ScanJobManager`: cancel/pause/progress tracking | tokio, scan progress | High | Complete |

### Core crate (`aisec-core`)

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `crates/aisec-core/Cargo.toml` | Core crate manifest | — | Low | Complete |
| `crates/aisec-core/src/lib.rs` | Crate root re-exports | error, logging | Low | Complete |
| `crates/aisec-core/src/error.rs` | `AisecError`, `AisecResult` shared error types | thiserror | Medium | Complete |
| `crates/aisec-core/src/logging.rs` | Shared tracing subscriber helpers | tracing | Medium | Complete |

### Auth crate (`aisec-auth`) — backend integration

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `crates/aisec-auth/Cargo.toml` | Auth crate manifest | aisec-storage, tokio | Low | Complete |
| `crates/aisec-auth/src/lib.rs` | Auth engine public API | engine, types | Low | Complete |
| `crates/aisec-auth/src/config.rs` | AuthEngineConfig, vault paths, Playwright bundle fields | — | Medium | Complete |
| `crates/aisec-auth/src/types.rs` | AuthMethod, AuthProfile, AuthConfig enums | serde | Medium | Complete |
| `crates/aisec-auth/src/engine.rs` | AuthEngine: profiles, interactive recording | playwright, session store | High | Complete |
| `crates/aisec-auth/src/cookies.rs` | Cookie parsing/serialization helpers | — | Medium | Complete |
| `crates/aisec-auth/src/mock.rs` | Mock auth engine for tests | — | Low | Test-only |
| `crates/aisec-auth/src/session/mod.rs` | Session module root | store | Low | Complete |
| `crates/aisec-auth/src/session/store.rs` | SessionStore: SQLite + vault file persistence | aisec-storage | High | Complete |
| `crates/aisec-auth/src/playwright/mod.rs` | Playwright client module | client, protocol | Low | Complete |
| `crates/aisec-auth/src/playwright/client.rs` | Spawns Node runner.mjs; env/cwd for bundle | tokio::process | High | Complete |
| `crates/aisec-auth/src/playwright/protocol.rs` | JSON protocol types for runner IPC | serde | Medium | Complete |
| `crates/aisec-auth/playwright/package.json` | Node deps: playwright | playwright npm | Low | Complete |
| `crates/aisec-auth/playwright/runner.mjs` | Playwright: begin/finish interactive login | playwright | High | Complete |
| `crates/aisec-auth/examples/record_replay.rs` | CLI example for auth recording | aisec-auth | Medium | Complete |

---

## Database

### Migrations

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `crates/aisec-storage/migrations/001_initial_schema.sql` | Core tables: projects, targets, scans, findings, payloads, attack_results, reports, models, plugins, FTS | — | High | Complete |
| `crates/aisec-storage/migrations/002_auth_schema.sql` | Auth tables: profiles, sessions, recordings | 001 | Medium | Complete |
| `crates/aisec-storage/migrations/003_endpoints.sql` | Discovery endpoints table + indexes | 001 | Low | Complete |

### Storage crate — core

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `crates/aisec-storage/Cargo.toml` | Storage crate manifest | sqlx, time, uuid | Low | Complete |
| `crates/aisec-storage/src/lib.rs` | Database, Repositories facade, re-exports | pool, repositories | Medium | Complete |
| `crates/aisec-storage/src/pool.rs` | SQLite pool open + migrate | sqlx | Medium | Complete |
| `crates/aisec-storage/src/models.rs` | Row structs + Create/Update DTOs | time, serde | High | Complete |
| `crates/aisec-storage/src/auth_models.rs` | Auth-specific row models | serde | Medium | Complete |
| `crates/aisec-storage/src/error.rs` | Storage error types | aisec-core | Low | Complete |
| `crates/aisec-storage/src/util.rs` | ID generation, timestamp helpers | uuid, time | Low | Complete |

### Storage crate — repository traits

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `crates/aisec-storage/src/repositories/mod.rs` | Repositories aggregate + trait exports | sqlite impls | Medium | Complete |
| `crates/aisec-storage/src/repositories/traits.rs` | Repository trait definitions (Project, Scan, …) | models | High | Complete |
| `crates/aisec-storage/src/repositories/auth_traits.rs` | Auth repository traits | auth_models | Medium | Complete |

### Storage crate — SQLite implementations

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `crates/aisec-storage/src/repositories/sqlite/mod.rs` | SQLite repo module + `Repositories` builder | all sqlite/*.rs | Medium | Complete |
| `crates/aisec-storage/src/repositories/sqlite/project.rs` | Project CRUD | sqlx | Medium | Complete |
| `crates/aisec-storage/src/repositories/sqlite/target.rs` | Target CRUD | sqlx | Medium | Complete |
| `crates/aisec-storage/src/repositories/sqlite/scan.rs` | Scan CRUD + playbook | sqlx | Medium | Complete |
| `crates/aisec-storage/src/repositories/sqlite/finding.rs` | Finding CRUD + FTS sync | sqlx | High | Complete |
| `crates/aisec-storage/src/repositories/sqlite/endpoint.rs` | Endpoint CRUD (discovery) | sqlx | Medium | Complete |
| `crates/aisec-storage/src/repositories/sqlite/attack_result.rs` | Attack result persistence | sqlx | Medium | Complete |
| `crates/aisec-storage/src/repositories/sqlite/report.rs` | Report metadata CRUD | sqlx | Medium | Complete |
| `crates/aisec-storage/src/repositories/sqlite/payload.rs` | Custom payload CRUD | sqlx | Medium | Complete |
| `crates/aisec-storage/src/repositories/sqlite/model.rs` | Local model registry CRUD | sqlx | Medium | Complete |
| `crates/aisec-storage/src/repositories/sqlite/plugin.rs` | Plugin registry CRUD | sqlx | Medium | Complete |
| `crates/aisec-storage/src/repositories/sqlite/auth.rs` | Auth profile/session/recording CRUD | sqlx | High | Complete |

---

## Discovery

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `crates/aisec-discovery/Cargo.toml` | Discovery crate manifest | reqwest, scraper | Low | Complete |
| `crates/aisec-discovery/src/lib.rs` | Public API re-exports | engine, types, config | Low | Complete |
| `crates/aisec-discovery/src/engine.rs` | `DiscoveryEngine::discover` orchestration | crawler, detectors, client | High | Complete |
| `crates/aisec-discovery/src/config.rs` | DiscoveryConfig, RetryConfig defaults | — | Medium | Complete |
| `crates/aisec-discovery/src/types.rs` | DiscoveredEndpoint, DiscoveryReport, CrawlStats | serde | Medium | Complete |
| `crates/aisec-discovery/src/client.rs` | HTTP client with retry | reqwest, retry | Medium | Complete |
| `crates/aisec-discovery/src/crawler.rs` | BFS HTML crawler | client, extract, url_policy | High | Complete |
| `crates/aisec-discovery/src/extract.rs` | Link/script/API URL extraction from HTML | scraper | High | Complete |
| `crates/aisec-discovery/src/retry.rs` | Retry policy for HTTP failures | — | Medium | Complete |
| `crates/aisec-discovery/src/url_policy.rs` | URL validation, origin, private network policy | url crate | Medium | Complete |
| `crates/aisec-discovery/src/browser.rs` | Optional Playwright browser capture | playwright runner | High | Partial (not used by IPC path) |
| `crates/aisec-discovery/src/detectors/mod.rs` | Detector module + probe orchestration | ai, api, graphql, openapi, paths | Medium | Complete |
| `crates/aisec-discovery/src/detectors/paths.rs` | Static path list for probes | — | Low | Complete |
| `crates/aisec-discovery/src/detectors/ai.rs` | AI/LLM route detection probes | client | Medium | Complete |
| `crates/aisec-discovery/src/detectors/api.rs` | REST API heuristics | client | Medium | Complete |
| `crates/aisec-discovery/src/detectors/graphql.rs` | GraphQL endpoint probes | client | Medium | Complete |
| `crates/aisec-discovery/src/detectors/openapi.rs` | OpenAPI/Swagger spec probes | client | Medium | Complete |
| `crates/aisec-discovery/playwright/package.json` | Discovery Playwright Node deps | playwright | Low | Complete |
| `crates/aisec-discovery/playwright/runner.mjs` | Browser capture runner (separate from auth) | playwright | Medium | Partial |
| `crates/aisec-discovery/examples/verify_target.rs` | CLI verification against live target | aisec-discovery | Medium | Complete |
| `crates/aisec-discovery/examples/browser_capture.rs` | Browser-based capture example | browser module | Medium | Complete |

---

## Attack

### Attack framework (`aisec-attack`)

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `crates/aisec-attack/Cargo.toml` | Attack crate manifest | aisec-payload, reqwest | Low | Complete |
| `crates/aisec-attack/src/lib.rs` | Public API: executor, registry, transport | submodules | Medium | Complete |
| `crates/aisec-attack/src/category.rs` | `AttackCategory` enum (9 categories) | serde | Low | Complete |
| `crates/aisec-attack/src/traits.rs` | `Attack` trait definition | types | Medium | Complete |
| `crates/aisec-attack/src/registry.rs` | AttackRegistry + builtin registration | attacks/* | Medium | Complete |
| `crates/aisec-attack/src/executor.rs` | `AttackExecutor`: run category attacks | registry, transport, payload | High | Complete |
| `crates/aisec-attack/src/orchestrator.rs` | Multi-phase attack orchestration | executor | High | Complete |
| `crates/aisec-attack/src/collector.rs` | ResultCollector / ResultSink | types | Medium | Complete |
| `crates/aisec-attack/src/lifecycle.rs` | AttackPhase lifecycle events | — | Medium | Complete |
| `crates/aisec-attack/src/types.rs` | AttackContext, Attempt, Evaluation types | serde | Medium | Complete |
| `crates/aisec-attack/src/error.rs` | AttackError types | aisec-core | Low | Complete |
| `crates/aisec-attack/src/target_auth.rs` | Apply descriptor auth to HTTP targets | serde_json | High | Complete |
| `crates/aisec-attack/src/scanner.rs` | PromptInjectionScanner (storage feature) | storage | High | Partial (feature-gated) |
| `crates/aisec-attack/src/payload/mod.rs` | Payload runner module | runner, mutator | Low | Complete |
| `crates/aisec-attack/src/payload/runner.rs` | PayloadRunner: load + mutate + dispatch | aisec-payload | High | Complete |
| `crates/aisec-attack/src/payload/mutator.rs` | PayloadMutator wrapper | aisec-payload | Medium | Complete |
| `crates/aisec-attack/src/transport/mod.rs` | Transport trait + exports | http, mock | Medium | Complete |
| `crates/aisec-attack/src/transport/http.rs` | HttpTransport: real HTTP probe requests | reqwest | High | Complete |
| `crates/aisec-attack/src/transport/mock.rs` | MockTransport for unit tests | — | Low | Test-only |
| `crates/aisec-attack/src/attacks/mod.rs` | Attack category module exports | all attacks | Low | Complete |
| `crates/aisec-attack/src/attacks/common.rs` | Shared attack evaluation helpers | types | Medium | Complete |
| `crates/aisec-attack/src/attacks/prompt_injection.rs` | Prompt injection attack impl | traits, payload | High | Complete |
| `crates/aisec-attack/src/attacks/system_prompt_extraction.rs` | System prompt extraction attack | traits | High | Complete |
| `crates/aisec-attack/src/attacks/jailbreak.rs` | Jailbreak attack | traits | High | Complete |
| `crates/aisec-attack/src/attacks/rag_leakage.rs` | RAG leakage attack | traits | High | Complete |
| `crates/aisec-attack/src/attacks/memory_poisoning.rs` | Memory poisoning attack | traits | High | Complete |
| `crates/aisec-attack/src/attacks/cross_user_leakage.rs` | Cross-user leakage attack | traits | High | Complete |
| `crates/aisec-attack/src/attacks/agent_goal_hijacking.rs` | Agent goal hijacking attack | traits | High | Complete |
| `crates/aisec-attack/src/attacks/tool_abuse.rs` | Tool abuse attack | traits | High | Complete |
| `crates/aisec-attack/src/attacks/mcp_abuse.rs` | MCP abuse attack | traits | High | Complete |
| `crates/aisec-attack/examples/scan_prompt_injection.rs` | CLI prompt injection scan example | aisec-attack | Medium | Complete |

### Payload library (`aisec-payload`)

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `crates/aisec-payload/Cargo.toml` | Payload crate manifest | serde | Low | Complete |
| `crates/aisec-payload/data/payloads.json` | Built-in prompt catalog (~20+ payloads, 9 categories) | — | Medium | Complete |
| `crates/aisec-payload/src/lib.rs` | Payload library public API | library, pipeline | Low | Complete |
| `crates/aisec-payload/src/types.rs` | Payload, category, tag types | serde | Medium | Complete |
| `crates/aisec-payload/src/error.rs` | Payload error types | — | Low | Complete |
| `crates/aisec-payload/src/library/mod.rs` | Embedded JSON payload loader | include_str payloads.json | Medium | Complete |
| `crates/aisec-payload/src/pipeline.rs` | Payload selection/generation pipeline | library, mutation | High | Complete |
| `crates/aisec-payload/src/mutation/mod.rs` | Mutation engine module | encodings | Medium | Complete |
| `crates/aisec-payload/src/mutation/encodings.rs` | Encoding mutations (base64, unicode, etc.) | — | Medium | Complete |

---

## Judge

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `crates/aisec-judge/Cargo.toml` | Judge crate manifest | aisec-models | Low | Complete |
| `crates/aisec-judge/src/lib.rs` | Public API: JudgeEngine, deterministic_engine | engine, types | Low | Complete |
| `crates/aisec-judge/src/engine.rs` | JudgeEngine: rules + regex + LLM consensus | evaluators, scoring | High | Complete |
| `crates/aisec-judge/src/types.rs` | JudgeRequest, JudgeVerdict, JudgeConfig | serde | Medium | Complete |
| `crates/aisec-judge/src/error.rs` | JudgeError types | — | Low | Complete |
| `crates/aisec-judge/src/consensus.rs` | ConsensusEngine report builder | types | Medium | Complete |
| `crates/aisec-judge/src/scoring.rs` | aggregate_confidence, consensus_vulnerable, severity | — | Medium | Complete |
| `crates/aisec-judge/src/roles.rs` | ModelRolePool (judge/classifier/attacker) | aisec-models runtime | Medium | Partial (not configured in app) |
| `crates/aisec-judge/src/prompts.rs` | LLM judge prompt templates | — | Medium | Complete |
| `crates/aisec-judge/src/mock_runtime.rs` | JsonMockRuntime for tests | aisec-models | Low | Test-only |
| `crates/aisec-judge/src/evaluators/mod.rs` | Evaluator module exports | rule, regex, llm | Low | Complete |
| `crates/aisec-judge/src/evaluators/rule.rs` | RuleBasedEvaluator (sync) | types | High | Complete |
| `crates/aisec-judge/src/evaluators/regex.rs` | RegexEvaluator with default patterns | regex crate | High | Complete |
| `crates/aisec-judge/src/evaluators/llm.rs` | LlmEvaluator async via InferenceRuntime | aisec-models | High | Partial (desktop uses deterministic only) |
| `crates/aisec-judge/examples/judge_with_local_model.rs` | Example: LLM judge with GGUF | aisec-models, aisec-judge | Medium | Complete |

---

## Report

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `crates/aisec-report/Cargo.toml` | Report crate manifest | — | Low | Complete |
| `crates/aisec-report/src/lib.rs` | Public API re-exports | engine, formatters | Low | Complete |
| `crates/aisec-report/src/engine.rs` | ReportingEngine: write reports to disk | formatters | High | Complete |
| `crates/aisec-report/src/types.rs` | ReportFormat, ReportKind, ReportInput, GeneratedReport | serde | Medium | Complete |
| `crates/aisec-report/src/data.rs` | ReportDataBuilder, StorageFindingRow | — | Medium | Complete |
| `crates/aisec-report/src/error.rs` | ReportError types | — | Low | Complete |
| `crates/aisec-report/src/charts.rs` | Chart rendering for HTML/PDF reports | — | Medium | Complete |
| `crates/aisec-report/src/recommendations.rs` | Mitigation recommendations + compliance refs | types | Medium | Complete |
| `crates/aisec-report/src/formatters/mod.rs` | Formatter registry (`formatter_for`) | html, pdf, json, sarif | Medium | Complete |
| `crates/aisec-report/src/formatters/html.rs` | HTML report formatter | charts, data | High | Complete |
| `crates/aisec-report/src/formatters/pdf.rs` | PDF report formatter | charts, data | High | Complete |
| `crates/aisec-report/src/formatters/json.rs` | JSON report formatter | data | Medium | Complete |
| `crates/aisec-report/src/formatters/sarif.rs` | SARIF 2.1.0 formatter | data | High | Complete |
| `crates/aisec-report/examples/generate_html_report.rs` | CLI HTML report generation example | aisec-report | Medium | Complete |
| `src-tauri/technical_html_report.html` | Sample/generated HTML report artifact | — | Low | Generated |

---

## Tests

### Frontend (Vitest)

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `vitest.config.ts` | Vitest runner configuration | vite | Low | Complete |
| `tests/frontend/errors.test.ts` | AppError / toAppError tests | `@/shared/errors` | Low | Complete |
| `tests/frontend/logger.test.ts` | Frontend logger tests | `@/shared/logging` | Low | Complete |
| `tests/frontend/targetDescriptor.test.ts` | Target form + auth schema tests | `targetDescriptor` | Medium | Complete |
| `tests/frontend/attackProfiles.test.ts` | Attack profile/category tests | `attackProfiles` | Low | Complete |
| `tests/frontend/discoveryPhases.test.ts` | Discovery phase label tests | `discoveryPhases` | Low | Complete |
| `tests/frontend/findingsFilters.test.ts` | Finding filter logic tests | `findingsFilters` | Low | Complete |
| `tests/frontend/reportDownloads.test.ts` | Report download helper tests | `reportDownloads` | Medium | Complete |

### Rust — integration workspace

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `tests/integration/Cargo.toml` | Integration test crate manifest | workspace crates | Low | Partial (missing tracing dep) |
| `tests/integration/src/lib.rs` | Test helpers | — | Low | Complete |
| `tests/integration/tests/core_smoke.rs` | Core crate smoke test | aisec-core | Low | Complete |
| `tests/integration/tests/mvp_flow.rs` | End-to-end MVP flow test | storage, commands | High | Complete |
| `tests/integration/tests/storage_persistence.rs` | SQLite persistence tests | aisec-storage | Medium | Complete |
| `tests/integration/tests/auth_engine.rs` | Auth engine integration test | aisec-auth | Medium | Partial (tokio process feature) |

### Rust — Tauri command tests

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `src-tauri/tests/database_integration.rs` | DB open + migration integration | aisec-storage | Medium | Complete |
| `src-tauri/tests/project_commands.rs` | Project command op tests | commands/projects | Medium | Complete |
| `src-tauri/tests/domain_commands.rs` | Domain command op tests | commands/domain | High | Complete |

### Rust — crate unit/integration tests

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `crates/aisec-attack/tests/integration.rs` | Attack executor integration | aisec-attack | High | Complete |
| `crates/aisec-attack/tests/scanner.rs` | Scanner tests | scanner module | Medium | Complete |
| `crates/aisec-attack/tests/target_auth_transport.rs` | Target auth header injection tests | target_auth, http | Medium | Complete |
| `crates/aisec-discovery/tests/integration.rs` | Discovery engine tests | aisec-discovery | High | Partial (network hang risk) |
| `crates/aisec-judge/tests/integration.rs` | Judge engine tests | aisec-judge | High | Partial (1 failing test) |
| `crates/aisec-report/tests/integration.rs` | Report generation tests | aisec-report | Medium | Complete |
| `crates/aisec-payload/tests/integration.rs` | Payload library tests | aisec-payload | Medium | Complete |
| `crates/aisec-models/tests/integration.rs` | Model manager tests | aisec-models | Medium | Complete |
| `crates/aisec-plugin-host/tests/sample_plugins.rs` | Sample plugin load tests | aisec-plugin-host | High | Partial (1 failing test) |

---

## Infrastructure

### Root workspace & build

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `Cargo.toml` | Rust workspace root, shared deps | all crates | Medium | Complete |
| `package.json` | npm scripts: dev, tauri, test, playwright setup/bundle | vite, tauri cli | Low | Complete |
| `vite.config.ts` | Vite + React + path aliases | @vitejs/plugin-react | Low | Complete |
| `tsconfig.json` | TypeScript config (app) | — | Low | Complete |
| `tsconfig.node.json` | TypeScript config (tooling) | — | Low | Complete |
| `AGENTS.md` | Cursor Cloud agent instructions | — | Low | Docs |
| `.gitignore` | Git ignore rules (incl. playwright bundle) | — | Low | Complete |

### Scripts

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `scripts/bundle-playwright-auth.sh` | Bundle Node LTS + Playwright + Chromium for release | npm, curl | High | Complete |
| `scripts/discovery-test-target.py` | Local HTTP target for discovery testing | Python | Medium | Complete |
| `scripts/auth-login-target.py` | Login form target for auth testing | Python | Medium | Complete |
| `scripts/spa-test-target.py` | SPA target with client-side routes | Python | Medium | Complete |
| `scripts/vuln-chatbot-target.py` | Vulnerable chatbot mock server | Python | Medium | Complete |
| `scripts/vuln-llm-target.py` | Vulnerable LLM API mock server | Python | Medium | Complete |

### Plugin SDK & samples

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `plugins/README.md` | Plugin directory overview | — | Low | Docs |
| `plugins/_template/aisec-plugin.toml` | Plugin manifest template | — | Low | Complete |
| `plugins/_template/plugin.py` | Python plugin stub | plugin-sdk-python | Low | Complete |
| `plugins/samples/README.md` | Sample plugins overview | — | Low | Docs |
| `plugins/samples/discovery-openapi-paths/aisec-plugin.toml` | Discovery sample manifest | — | Low | Complete |
| `plugins/samples/discovery-openapi-paths/plugin.py` | OpenAPI path discovery plugin | SDK | Medium | Complete |
| `plugins/samples/attack-delimiter-injection/aisec-plugin.toml` | Attack sample manifest | — | Low | Complete |
| `plugins/samples/attack-delimiter-injection/plugin.js` | Delimiter injection attack plugin | SDK JS | Medium | Complete |
| `plugins/samples/judge-keyword/aisec-plugin.toml` | Judge sample manifest | — | Low | Complete |
| `plugins/samples/judge-keyword/plugin.py` | Keyword judge plugin | SDK | Medium | Complete |
| `plugins/samples/report-markdown-summary/aisec-plugin.toml` | Report sample manifest | — | Low | Complete |
| `plugins/samples/report-markdown-summary/plugin.js` | Markdown summary report plugin | SDK JS | Medium | Complete |
| `packages/plugin-sdk-python/pyproject.toml` | Python SDK package config | — | Low | Complete |
| `packages/plugin-sdk-python/README.md` | Python SDK docs | — | Low | Docs |
| `packages/plugin-sdk-python/aisec_plugin/__init__.py` | Python SDK package root | — | Low | Complete |
| `packages/plugin-sdk-python/aisec_plugin/base.py` | Base plugin class | protocol | Medium | Complete |
| `packages/plugin-sdk-python/aisec_plugin/protocol.py` | JSON-RPC protocol types | — | Medium | Complete |
| `packages/plugin-sdk-python/aisec_plugin/discovery.py` | Discovery plugin helpers | base | Medium | Complete |
| `packages/plugin-sdk-python/aisec_plugin/attack.py` | Attack plugin helpers | base | Medium | Complete |
| `packages/plugin-sdk-python/aisec_plugin/judge.py` | Judge plugin helpers | base | Medium | Complete |
| `packages/plugin-sdk-python/aisec_plugin/report.py` | Report plugin helpers | base | Medium | Complete |
| `packages/plugin-sdk-js/package.json` | JS SDK package config | — | Low | Complete |
| `packages/plugin-sdk-js/src/index.js` | JS SDK entry | base, protocol | Low | Complete |
| `packages/plugin-sdk-js/src/base.js` | Base plugin class | protocol | Medium | Complete |
| `packages/plugin-sdk-js/src/protocol.js` | JSON-RPC protocol | — | Medium | Complete |
| `packages/plugin-sdk-js/src/discovery.js` | Discovery helpers | base | Medium | Complete |
| `packages/plugin-sdk-js/src/attack.js` | Attack helpers | base | Medium | Complete |
| `packages/plugin-sdk-js/src/judge.js` | Judge helpers | base | Medium | Complete |
| `packages/plugin-sdk-js/src/report.js` | Report helpers | base | Medium | Complete |

### Supporting crates — models (`aisec-models`)

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `crates/aisec-models/Cargo.toml` | Models crate manifest | llama-cpp bindings | Low | Complete |
| `crates/aisec-models/src/lib.rs` | Model manager public API | manager, runtime | Low | Complete |
| `crates/aisec-models/src/types.rs` | Model metadata types | serde | Medium | Complete |
| `crates/aisec-models/src/error.rs` | Model error types | — | Low | Complete |
| `crates/aisec-models/src/manager.rs` | ModelManager: registry + verify | registry, download | High | Complete |
| `crates/aisec-models/src/registry.rs` | In-memory model registry | types | Medium | Complete |
| `crates/aisec-models/src/verify.rs` | GGUF checksum verification | — | Medium | Complete |
| `crates/aisec-models/src/download/mod.rs` | Download module | huggingface, manager | Low | Complete |
| `crates/aisec-models/src/download/manager.rs` | Download orchestration | huggingface | High | Complete |
| `crates/aisec-models/src/download/huggingface.rs` | HuggingFace HTTP download | reqwest | High | Complete |
| `crates/aisec-models/src/hardware/mod.rs` | Hardware detection module | detect | Low | Complete |
| `crates/aisec-models/src/hardware/detect.rs` | CPU/GPU capability detection | — | Medium | Complete |
| `crates/aisec-models/src/runtime/mod.rs` | InferenceRuntime trait + exports | llama_cpp, mock | Medium | Complete |
| `crates/aisec-models/src/runtime/llama_cpp.rs` | llama.cpp subprocess runtime | — | High | Complete |
| `crates/aisec-models/src/runtime/llama_inproc.rs` | In-process llama runtime | — | High | Complete |
| `crates/aisec-models/src/runtime/mock.rs` | Mock inference for tests | — | Low | Test-only |
| `crates/aisec-models/examples/run_local_model.rs` | CLI local model inference example | aisec-models | Medium | Complete |

### Supporting crates — fingerprint (`aisec-fingerprint`)

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `crates/aisec-fingerprint/Cargo.toml` | Fingerprint crate manifest | — | Low | Complete |
| `crates/aisec-fingerprint/src/lib.rs` | Fingerprint engine public API | engine | Low | Complete |
| `crates/aisec-fingerprint/src/engine.rs` | Provider fingerprint orchestration | rules, evaluator | High | Complete |
| `crates/aisec-fingerprint/src/evaluator.rs` | Rule evaluation against HTTP responses | rules | Medium | Complete |
| `crates/aisec-fingerprint/src/scoring.rs` | Confidence scoring | — | Medium | Complete |
| `crates/aisec-fingerprint/src/openapi.rs` | OpenAPI-based fingerprint hints | — | Medium | Complete |
| `crates/aisec-fingerprint/src/types.rs` | Fingerprint result types | serde | Low | Complete |
| `crates/aisec-fingerprint/src/rules/mod.rs` | Provider rules module | providers | Low | Complete |
| `crates/aisec-fingerprint/src/rules/providers/mod.rs` | Provider rule registry | all providers | Medium | Complete |
| `crates/aisec-fingerprint/src/rules/providers/openai.rs` | OpenAI provider rules | — | Medium | Complete |
| `crates/aisec-fingerprint/src/rules/providers/anthropic.rs` | Anthropic provider rules | — | Medium | Complete |
| `crates/aisec-fingerprint/src/rules/providers/azure_openai.rs` | Azure OpenAI rules | — | Medium | Complete |
| `crates/aisec-fingerprint/src/rules/providers/bedrock.rs` | AWS Bedrock rules | — | Medium | Complete |
| `crates/aisec-fingerprint/src/rules/providers/gemini.rs` | Google Gemini rules | — | Medium | Complete |
| `crates/aisec-fingerprint/src/rules/providers/ollama.rs` | Ollama rules | — | Medium | Complete |
| `crates/aisec-fingerprint/src/rules/providers/litellm.rs` | LiteLLM proxy rules | — | Medium | Complete |
| `crates/aisec-fingerprint/src/rules/providers/vllm.rs` | vLLM rules | — | Medium | Complete |
| `crates/aisec-fingerprint/examples/detect_ai.rs` | CLI AI provider detection example | aisec-fingerprint | Medium | Complete |

### Supporting crates — plugin host (`aisec-plugin-host`)

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `crates/aisec-plugin-host/Cargo.toml` | Plugin host manifest | — | Low | Complete |
| `crates/aisec-plugin-host/src/lib.rs` | PluginHost public API | manager | Low | Complete |
| `crates/aisec-plugin-host/src/manager.rs` | PluginManager: load, enable, invoke | manifest, sandbox | High | Complete |
| `crates/aisec-plugin-host/src/manifest.rs` | Parse aisec-plugin.toml | toml | Medium | Complete |
| `crates/aisec-plugin-host/src/lifecycle.rs` | Plugin lifecycle state machine | types | Medium | Complete |
| `crates/aisec-plugin-host/src/permissions.rs` | Capability permission checks | manifest | Medium | Complete |
| `crates/aisec-plugin-host/src/types.rs` | Plugin descriptor types | serde | Medium | Complete |
| `crates/aisec-plugin-host/src/error.rs` | Plugin error types | — | Low | Complete |
| `crates/aisec-plugin-host/src/sandbox/mod.rs` | Sandbox module | runner | Low | Complete |
| `crates/aisec-plugin-host/src/sandbox/runner.rs` | Subprocess plugin runner (Python/Node) | — | High | Complete |

### Tauri generated assets

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `src-tauri/gen/schemas/acl-manifests.json` | Tauri ACL schema (generated) | tauri build | Low | Generated |
| `src-tauri/gen/schemas/capabilities.json` | Capabilities schema (generated) | tauri build | Low | Generated |
| `src-tauri/gen/schemas/desktop-schema.json` | Desktop permission schema | tauri build | Low | Generated |
| `src-tauri/gen/schemas/macOS-schema.json` | macOS permission schema | tauri build | Low | Generated |

### Documentation

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `docs/PROJECT_CURRENT_STATE.md` | Full architecture audit (current state) | — | — | Docs |
| `docs/PROJECT_FILE_INDEX.md` | This file index | — | — | Docs |
| `docs/ARCHITECTURE.md` | System architecture overview | — | — | Docs |
| `docs/DATABASE.md` | Database schema reference | migrations | — | Docs |
| `docs/AUTH.md` | Auth engine + Playwright setup | — | — | Docs |
| `docs/DISCOVERY.md` | Discovery engine design | — | — | Docs |
| `docs/DISCOVERY_STATUS.md` | Discovery implementation status | — | — | Docs |
| `docs/DISCOVERY_VERIFICATION_REPORT.md` | Discovery verification results | — | — | Docs |
| `docs/ATTACK.md` | Attack framework design | — | — | Docs |
| `docs/ATTACK_STATUS.md` | Attack implementation status | — | — | Docs |
| `docs/JUDGE.md` | Judge engine design | — | — | Docs |
| `docs/JUDGE_STATUS.md` | Judge implementation status | — | — | Docs |
| `docs/REPORT.md` | Report engine design | — | — | Docs |
| `docs/PAYLOAD.md` | Payload library design | — | — | Docs |
| `docs/MODELS.md` | Local models design | — | — | Docs |
| `docs/FINGERPRINT.md` | Fingerprint engine design | — | — | Docs |
| `docs/PLUGINS.md` | Plugin system design | — | — | Docs |
| `docs/PROJECT_STRUCTURE.md` | Repo layout (partially outdated) | — | — | Docs |
| `docs/PROJECT_BACKEND_STATUS.md` | Backend integration status | — | — | Docs |
| `docs/STATUS.md` | General project status | — | — | Docs |
| `docs/MVP_CHECKLIST.md` | MVP checklist | — | — | Docs |
| `docs/MVP_GAP_ANALYSIS.md` | MVP gap analysis | — | — | Docs |
| `docs/MVP_EXECUTION_PLAN.md` | MVP execution plan | — | — | Docs |
| `docs/MVP_VALIDATION_REPORT.md` | MVP validation report | — | — | Docs |
| `docs/MOCK_INVENTORY.md` | Mock data inventory | — | — | Docs |
| `docs/MOCK_REMOVAL_PLAN.md` | Mock removal plan | — | — | Docs |
| `docs/APP_INTEGRATION_GAP_REPORT.md` | Frontend/backend integration gaps | — | — | Docs |
| `docs/REAL_IMPLEMENTATION_AUDIT.md` | Real vs mock implementation audit | — | — | Docs |
| `docs/SCAN_WIZARD_REDESIGN.md` | Scan wizard redesign notes | — | — | Docs |
| `docs/UI_REFACTOR_PLAN.md` | UI refactor plan | — | — | Docs |
| `docs/UX_CONSISTENCY_REPORT.md` | UX consistency audit | — | — | Docs |
| `docs/AUDIT_REPORT.md` | General audit report | — | — | Docs |
| `docs/CODE_HYGIENE_REPORT.md` | Code hygiene report | — | — | Docs |

### Prompts & agent context

| Path | Purpose | Dependencies | Complexity | Status |
|------|---------|--------------|------------|--------|
| `prompts/00-architecture.md` | Architecture prompt/context for agents | — | Low | Docs |

---

## Cross-reference: files by primary engine

| Group | File count (important) | Entry points |
|-------|------------------------|--------------|
| Frontend | ~90 | `src/main.tsx`, `AppRouter.tsx`, `AppStore.tsx` |
| Backend | ~35 | `src-tauri/src/lib.rs`, `commands/*.rs` |
| Database | ~20 | `aisec-storage/src/lib.rs`, migrations |
| Discovery | ~18 | `aisec-discovery/src/engine.rs`, `commands/discovery.rs` |
| Attack | ~35 | `aisec-attack/src/executor.rs`, `commands/attack.rs`, `payloads.json` |
| Judge | ~14 | `aisec-judge/src/engine.rs` (called from `attack.rs`) |
| Report | ~14 | `aisec-report/src/engine.rs`, `commands/domain.rs` |
| Tests | ~25 | `vitest.config.ts`, `tests/integration/tests/mvp_flow.rs` |
| Infrastructure | ~80 | `Cargo.toml`, `scripts/`, `docs/`, plugin SDK |

---

*End of index.*
