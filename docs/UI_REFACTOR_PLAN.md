# AISec UI Refactor — Implementation Task Breakdown

> Source spec: `docs/SCAN_WIZARD_REDESIGN.md` (approved).
> Rules: every task ≤ **4h**, has **dependencies** + **acceptance criteria**, grouped by layer
> (**Backend**, **IPC**, **Frontend**). Reuse existing engines/SQLite — **no mock data, no fake backend**.
> This is planning only; no code in this document.

Task IDs: `B*` backend (Rust `src-tauri`/crates), `I*` IPC layer (TS client wrappers, DTO types, event
listeners in `src/shared/ipc`), `F*` frontend (React UI). Dependencies reference IDs across layers.
"EXISTS" = already implemented today; such commands need no `B*` task, only `I*`/`F*` wiring.

---

## Backend tasks (Rust)

### B1 — `project_update` command (1h)
- Expose `ProjectRepository::update` (already exists) as a Tauri command + DTO passthrough; register in `lib.rs`.
- **Deps:** none.
- **Accept:** updating name/description persists to SQLite and is returned; integration test covers update.

### B2 — Target get/update/delete commands (2h)
- Expose `target_get`, `target_update`, `target_delete` over existing `TargetRepository` methods.
- **Deps:** none.
- **Accept:** each operates on SQLite; delete removes the row; tests for get/update/delete.

### B3 — `scan_get` + `scan_list_all` commands (2h)
- `scan_get(id)` (repo `get` exists). `scan_list_all()` = scans across all projects (new repo method
  `ScanRepository::list_all` or aggregate over projects).
- **Deps:** none.
- **Accept:** monitor can fetch one scan and all scans; ordered newest-first; test for both.

### B4 — Endpoint `source` column migration (1h)
- Migration `004_endpoint_source.sql`: `ALTER TABLE endpoints ADD COLUMN source TEXT NOT NULL DEFAULT 'discovery'`;
  add `source` to `Endpoint` model + `CreateEndpoint`.
- **Deps:** none.
- **Accept:** discovery-inserted rows default `discovery`; column readable via existing list; storage test green.

### B5 — Manual `endpoint_create` command (2h)
- Build absolute URL from the target origin + entered `path`; persist via `EndpointRepository::create` with
  `source="manual"`, given `method`. Associate to the wizard's discovery scan + target.
- **Deps:** B4.
- **Accept:** a manual endpoint persists with `source="manual"` and appears in `endpoint_list`; test covers it.

### B6 — Findings read APIs (1.5h)
- Expose `finding_list_by_project(projectId)` and `finding_search(query, limit)` (repo methods exist).
- **Deps:** none.
- **Accept:** both return real rows from SQLite; search matches title/category; tests green.

### B7 — `finding_export(scanId|projectId, format)` command (2h)
- Serialize findings to CSV and JSON (in-memory) and return content (or write+return path like reports).
- **Deps:** B6.
- **Accept:** export of a scan's findings yields valid CSV/JSON containing the real rows; test asserts content.

### B8 — Auth: basic/API-key descriptor handling (1h)
- Persist Basic (username/password) and API-key (header name/value) into the target `descriptor_json.auth`;
  ensure the attack job feeds them to `AttackTarget` (`with_auth`/headers).
- **Deps:** B2.
- **Accept:** a target with API-key auth sends the header on attack requests (verified against a test endpoint).

### B9 — Auth: SSO via Playwright (`auth_record_session`) (4h)
- Wrap `aisec-auth` Playwright login capture; persist to `auth_profiles`/`auth_sessions`; return `sessionId`;
  link session to target descriptor.
- **Deps:** B2; `aisec-auth` reachable from `src-tauri` (add dep).
- **Accept:** launching SSO records a session row; the session token/cookies are reused by the scan job; manual test.

### B10 — Scan Job Manager foundation (4h)
- New module: `AppState.jobs: Arc<Mutex<HashMap<ScanId, JobHandle>>>`; `JobHandle { task, cancel_token, paused, progress }`;
  `ScanProgress { status, current_endpoint, current_test, completed, total, findings, started_at }`.
- **Deps:** none.
- **Accept:** unit test can register/lookup/remove a job handle and mutate progress safely (concurrent access).

### B11 — Reusable single-unit attack+judge runner (3h)
- Extract the existing `attack_run_prompt_injection` logic into `run_category_on_endpoint(state, scan, endpoint, category)`
  that executes `aisec-attack` + `aisec-judge` and persists `attack_results` + `findings`.
- **Deps:** none (refactor of existing code).
- **Accept:** existing prompt-injection behavior unchanged (same findings); function reused by old command + B12.

### B12 — `scan_start` background job (4h)
- Validate inputs; create/link scan (`status=running`, `started_at`); `tokio::spawn` a job iterating
  selected endpoints × categories via B11, updating `ScanProgress`; return `{ scanId }` immediately.
- **Deps:** B10, B11.
- **Accept:** command returns in <200ms while the scan runs in background; findings appear in SQLite as it progresses.

### B13 — Scan progress events + persistence (4h)
- Emit `scan://progress`, `scan://phase`, `scan://finding`, `scan://completed`; write coarse progress into the scan
  record so the Monitor survives an app restart.
- **Deps:** B12.
- **Accept:** a subscriber receives ordered events; on completion scan is `completed` with `completed_at`; restart shows last state.

### B14 — `scan_status` command (1h)
- Return current `ScanProgress` for a scan (live job or last persisted).
- **Deps:** B10, B12.
- **Accept:** returns accurate live status during a run and final status after.

### B15 — Pause / Resume / Stop (3h)
- `scan_pause/resume/stop` toggle the paused flag / cancel token; the job checks **between units** (cooperative).
- **Deps:** B12.
- **Accept:** pause halts new requests within one unit; resume continues; stop ends with `status=cancelled`; no mid-request corruption.

### B16 — Target health check command (2h)
- `target_health(targetId)` → `Reachable | AuthRequired | Offline` via a real HTTP probe (HEAD/GET, short timeout).
- **Deps:** B2.
- **Accept:** returns correct state for an online endpoint, a 401-protected one, and an unreachable host.

### B17 — Report format passthrough + binary delivery (2h)
- Ensure `report_generate` honors `format ∈ {html,pdf,json,sarif}` (engine already supports all). Add binary-safe
  delivery for PDF: `report_read_bytes(id)` or document use of existing binary-safe `report_export`.
- **Deps:** none.
- **Accept:** generating PDF produces a valid PDF file; HTML/JSON/SARIF unaffected; download returns intact bytes.

### B18 (optional) — Discovery live progress (4h)
- Add a progress callback to `aisec-discovery`; emit `discovery://progress` phases (crawl/JS/API/GraphQL/OpenAPI).
- **Deps:** none.
- **Accept:** Step 3 receives phase events during a real crawl. *Optional polish — wizard works without it (indeterminate phases).*

---

## IPC tasks (TS client + events)

> Thin typed wrappers + DTO types in `src/shared/ipc`. Tauri auto-maps snake_case→camelCase args.

### I1 — Projects/Targets IPC (1.5h)
- Wrappers + DTO types: `updateProject`, `getTarget`, `updateTarget`, `deleteTarget`.
- **Deps:** B1, B2.
- **Accept:** typed calls succeed against the commands; types match DTOs; `tsc` clean.

### I2 — Manual endpoint IPC (1h)
- `createEndpoint(scanId, targetId, method, path)` wrapper + `EndpointDto.source` field.
- **Deps:** B5.
- **Accept:** call returns the persisted endpoint with `source="manual"`.

### I3 — Scan lifecycle IPC (2h)
- `startScan(...)`, `getScan(id)`, `listAllScans()`, `getScanStatus(id)` wrappers + DTOs (`ScanProgressDto`).
- **Deps:** B3, B12, B14.
- **Accept:** `startScan` returns `{scanId}` immediately; status typed; `tsc` clean.

### I4 — Scan control IPC (1h)
- `pauseScan/resumeScan/stopScan` wrappers.
- **Deps:** B15.
- **Accept:** calls toggle backend state (observable via status).

### I5 — Tauri event subscription utility (3h)
- Typed `subscribeScanEvents(handlers)` over `@tauri-apps/api/event` for `scan://progress|phase|finding|completed`
  (+ `discovery://progress` if B18). Unsubscribe on unmount. Poll fallback via `getScanStatus`.
- **Deps:** B13.
- **Accept:** a React hook receives live updates for a running scan; cleans up listeners; degrades to polling if events absent.

### I6 — Findings IPC (1.5h)
- `listFindingsByProject`, `searchFindings`, `exportFindings(scanId, format)` wrappers.
- **Deps:** B6, B7.
- **Accept:** typed; export returns content/path; `tsc` clean.

### I7 — Auth IPC (1h)
- `recordAuthSession(targetId, kind, credentials?)` wrapper + auth DTOs.
- **Deps:** B8, B9.
- **Accept:** SSO call returns a `sessionId`; basic/api-key calls persist descriptor auth.

### I8 — Reports IPC (1.5h)
- `generateReport(projectId, scanId, format, kind)` already exists; add `readReportBytes`/binary export wrapper for PDF.
- **Deps:** B17.
- **Accept:** PDF download yields a valid file; other formats unchanged.

### I9 — Target health IPC (1h)
- `getTargetHealth(targetId)` wrapper + `TargetHealth` type.
- **Deps:** B16.
- **Accept:** returns Reachable/AuthRequired/Offline; typed.

---

## Frontend tasks (React)

### F1 — Sidebar + Settings/Models relocation (2h)
- Remove Discovery/Attacks/Models nav items; add **Scans**; render Models as a Settings subsection/tab.
- **Deps:** none.
- **Accept:** sidebar shows the 7 final items; Models reachable under Settings; no dead nav.

### F2 — Routing shell + deep linking (3h)
- Add routes `/scans`, `/scans/new`, `/scans/:scanId`, `/projects/:projectId`, `/settings/models`; redirect
  `/discovery`,`/attacks`,`/models`; param resolution scaffolding (loading/empty/error).
- **Deps:** F1.
- **Accept:** all routes render placeholders; pasted deep links resolve params; old routes redirect.

### F3 — Projects CRUD + create→wizard redirect (3h)
- Edit modal (reuse `Modal`), delete confirm + toast; on **Create Project** success → `navigate('/scans/new', {state:{projectId}})`.
- **Deps:** F2, I1.
- **Accept:** create/edit/delete persist via IPC with loading/error; successful create lands on the wizard with project pre-set.

### F4 — Project detail page (4h)
- `/projects/:projectId` with tabs (Targets/Scans/Findings/Reports) reusing existing lists + "New Scan" CTA.
- **Deps:** F2.
- **Accept:** detail loads real entity by id; tabs show that project's data; New Scan opens wizard locked to the project.

### F5 — Scan Wizard shell + draft provider (4h)
- `/scans/new` layout, `WizardProgress` (6 steps), `ScanWizardProvider` (reducer draft, step nav, guards).
- **Deps:** F2.
- **Accept:** stepper navigates Back/Next with validation gating; draft persists across steps within the route.

### F6 — Step 1 Project Selection (2h)
- Scenario A: pre-selected + **locked** (from route state); Scenario B: dropdown (must select). Show name/description.
- **Deps:** F5, I1.
- **Accept:** arriving from Projects locks the selector; direct entry requires a selection before Next.

### F7 — Step 2 Target + Authentication (4h)
- URL input; auth selector None/Basic/SSO/API-key with conditional fields; **persist target** (`target_create`,
  EXISTS) associated to project; SSO → `recordAuthSession`. Back / Start Discovery.
- **Deps:** F5, I7 (target_create EXISTS).
- **Accept:** target persists to SQLite with the chosen auth in descriptor; SSO records a session; Next requires a saved target.

### F8 — Step 3 Discovery + endpoint selection (4h)
- Run `discovery_run` (EXISTS); show phase chips (crawl/JS/API/GraphQL/OpenAPI); endpoint table (Method/Endpoint/
  Confidence/Source) with per-row checkbox; live phases via I5 if available else indeterminate.
- **Deps:** F5, I5 (live optional; discovery_run EXISTS).
- **Accept:** real endpoints appear from SQLite; selection state tracked; at least one selectable to proceed.

### F9 — Step 3 Manual endpoints (3h)
- "Manual Endpoints" section, Add Endpoint (Method+Path) → `createEndpoint`; **Manual** badge; selectable like discovered.
- **Deps:** F8, I2.
- **Accept:** added endpoint persists, shows Manual badge, and is selectable for the scan.

### F10 — Step 4 Attack Planning (4h)
- Profile presets (Quick/Standard/Deep/Custom) mapped to real `AttackCategory` values; category accordion with
  test count + names (from `aisec-payload`) + per-test toggle; **Estimated Requests/Runtime** (client-side from real
  payload counts × selected endpoints). Back / Start Scan.
- **Deps:** F5.
- **Accept:** presets select the correct categories; toggling tests updates the estimate; Custom lets user pick categories.

### F11 — Step 5 Submission (3h)
- On Start Scan → `startScan` (returns immediately); show **Scan Submitted** (Scan ID/Target/Status/Progress) +
  actions (Open Monitor / Create Another / Go to Findings / Go to Targets); explain background execution. **No waiting trap.**
- **Deps:** F5, I3.
- **Accept:** UI never blocks on a spinner; scanId shown; each action routes correctly.

### F12 — Scans page = Scan Monitor list (4h)
- Cards: Scan ID/Project/Target/Status/Progress/Findings/Started/Duration + Pause/Resume/Stop; live via I5.
- **Deps:** I3, I4, I5.
- **Accept:** all scans listed from SQLite; running scans update live; controls reflect in backend status.

### F13 — Scan Monitor detail (4h)
- `/scans/:scanId` live view: Current Endpoint/Current Test/Current Progress + controls; deep-linkable.
- **Deps:** I3, I4, I5.
- **Accept:** opening a running scan shows real-time current endpoint/test; controls work; reload resumes from last state.

### F14 — Findings page (read-only) (3h)
- Remove scanning actions; add Filter/Search/Export; columns Project/Scan/Severity/Category/Endpoint/Status (+ Judge Verdict).
- **Deps:** I6.
- **Accept:** no mutation actions; filters/search query real data; export downloads real findings.

### F15 — Reports page (read-only) (3h)
- Columns Project/Scan/Generated; actions Download HTML / Download PDF / Export SARIF via real engine output.
- **Deps:** I8.
- **Accept:** HTML/PDF/SARIF downloads are valid files containing real findings; no generation logic on this page beyond invoking the command.

### F16 — Targets page health (3h)
- Columns Project/URL/Auth/Last Scan/Status (New/Scanned/Running/Failed) + health badge (Reachable/Auth Required/Offline).
- **Deps:** I9.
- **Accept:** status derives from real scans; health badge reflects `getTargetHealth`; loading/error states present.

### F17 — Remove legacy Discovery/Attacks pages (2h)
- Delete `DiscoveryPage`/`AttacksPage` + routes + dead store actions once the wizard/monitor replace them.
- **Deps:** F8, F9, F12 (wizard+monitor live).
- **Accept:** app builds with no references to removed pages; no orphan routes/imports.

---

## Suggested sequencing (phase → tasks)

| Phase | Tasks | Outcome |
|-------|-------|---------|
| 0 Shell | F1, F2 | New nav + routes (placeholders) |
| 1 Projects | B1, I1, F3, F4 | Projects CRUD + detail + create→wizard |
| 2 Wizard 1–3 | B4, B5, I2, B8, B9, I7, F5, F6, F7, F8, F9 | Target+auth+discovery+endpoints persisted |
| 3 Wizard 4 | F10 | Attack plan + estimates |
| 4 Scan engine + Monitor | B10, B11, B12, B13, B14, B15, I3, I4, I5, F11, F12, F13 | Background scans + live monitor (**core**) |
| 5 Outcomes | B6, B7, B16, B17, I6, I8, I9, F14, F15, F16 | Read-only Findings/Reports + Target health |
| 6 Cleanup | F17 | Remove legacy pages |
| Optional | B18 | Live discovery phase events |

**Critical path:** B10 → B11 → B12 → B13 → (B14/B15) → I3/I5 → F11/F12/F13. This is the largest net-new work
(the background Scan Job Manager); everything else reuses existing commands/engines.
