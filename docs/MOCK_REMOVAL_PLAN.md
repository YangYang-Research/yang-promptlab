# AISec — Mock / Placeholder Removal Plan

> **Trạng thái (2026-06-12):** Một phần đã thực hiện (PR #19: xóa `src/shared/mock/data.ts`,
> IPC-backed store). Các mục còn lại trong tài liệu vẫn có thể áp dụng.

**Author role:** Principal Software Architect
**Date:** 2026-06-11
**Inputs:** `docs/MVP_EXECUTION_PLAN.md`, `docs/REAL_IMPLEMENTATION_AUDIT.md`
**Method:** Repository-wide search for `mock`, `fake`, `placeholder`, `stub`, `TODO`/`FIXME`,
`unimplemented!`, `todo!`, and related markers (`let _ =` discards, hardcoded fallbacks). Verified by
reading each hit. No code modified.

> **Headline:** there are **zero `todo!()` and zero `unimplemented!()`** macros in the codebase, and
> only **one `TODO` comment**. The real "mock surface" is concentrated in **(a) the mock-fed React
> store** and **(b) a set of legitimate test doubles**. Most "placeholders" are `let _ = …` discards
> and hardcoded fallback values, not stub macros.

---

## Priority legend (aligned to the MVP scan flow)

| Priority | Meaning |
|----------|---------|
| **P0** | Blocks the MVP real scan (`docs/MVP_EXECUTION_PLAN.md` B1-B3). Must remove/replace. |
| **P1** | Affects MVP correctness or a required-module happy path (B4-B8). |
| **P2** | Quality/feature gap in a shipped crate; not on the MVP path. |
| **P3** | Legitimate test double / by-design abstract stub / test fixture. **Keep** (replace only if scope expands). |

### Counts

| Category | Items | Typical priority |
|----------|-------|------------------|
| A. Frontend mock data + store | 11 | P0 |
| B. Frontend dead action buttons | ~30 (key 4 are P0) | P0 / P2 |
| C. Tauri backend skeleton (missing real commands/state) | 3 | P0 |
| D. Production `let _ =` discards & hardcoded fallbacks | 7 | P1 / P2 |
| E. Domain behavioral placeholders (dead config, shallow logic) | 9 | P1 / P2 |
| F. Test-only mock runtimes/drivers/transports | 4 | P3 (keep) |
| G. Test fixtures (`stub`/`fake` strings) | 4 | P3 (keep) |
| H. Plugin SDK abstract stubs (by design) | 8 | P3 |
| I. Sample plugin hardcoded data | 1+ | P3 |
| J. TODO markers | 1 | P1 |
| **—** | `todo!()` / `unimplemented!()` | **0** |

---

## A. Frontend mock data + store — **P0** (MVP blocker B3)

The entire desktop UI renders from a single hardcoded mock module; the store is seeded with it and
never hydrates from the backend.

| File | Function / symbol | Real implementation required | Priority |
|------|-------------------|------------------------------|----------|
| `src/shared/mock/data.ts` | `mockProjects` | Load via `project_list` IPC → SQLite `projects` | **P0** |
| `src/shared/mock/data.ts` | `mockTargets` | Load via a `target_list` IPC → SQLite `targets` | **P0** |
| `src/shared/mock/data.ts` | `mockFindings` | Load via `findings_list` IPC → SQLite `findings` | **P0** |
| `src/shared/mock/data.ts` | `mockReports` | Load via `report_list` / result of `report_generate` | **P0** |
| `src/shared/mock/data.ts` | `mockDiscoveryJobs` | Derive from `scan_run` result/status | P1 |
| `src/shared/mock/data.ts` | `mockAttackRuns` | Derive from `scan_run` / attack results | P1 |
| `src/shared/mock/data.ts` | `mockModels` | Load from `aisec-models` registry via IPC | P2 (optional module) |
| `src/shared/mock/data.ts` | `mockActivity` | Derive from scan/finding events or audit log | P2 |
| `src/shared/mock/data.ts` | `computeDashboardStats`, `severityCounts` | Recompute from real findings/scan data | P1 |
| `src/app/store/AppStore.tsx` | `initialState` (seeded from all 8 mocks, lines 24-31) | Start empty; populate via IPC calls on mount; add reducer actions for create/list | **P0** |
| `src/features/dashboard/DashboardPage.tsx` | imports `severityCounts` from mock (line 4) | Use stats computed from real findings | P1 |

**Outcome required:** delete `src/shared/mock/data.ts` (or gate behind a dev-only flag), seed the
store from IPC, and ensure the "Connected" badge reflects a real DB-backed backend.

---

## B. Frontend dead action buttons — **P0 for the 4 MVP actions, P2 otherwise**

`<Button>`s with no `onClick`/handler. The four MVP-critical actions are P0; the rest are deferred.

| File | Function / control | Real implementation required | Priority |
|------|--------------------|------------------------------|----------|
| `src/features/projects/ProjectsPage.tsx:83` | "New Project" | Form → `project_create` IPC → refresh list | **P0** |
| `src/features/targets/TargetsPage.tsx:89` | "Add Target" | Form → `target_create` IPC → refresh list | **P0** |
| `src/features/discovery/DiscoveryPage.tsx:21` | "Start Scan" | Trigger `scan_run` IPC (crawl+discover+attack+evaluate) | **P0** |
| `src/features/reports/ReportsPage.tsx:84` | "Generate Report" | `report_generate` IPC → open file path | **P0** |
| `src/features/dashboard/DashboardPage.tsx:33` | "New Project" | Same as ProjectsPage action | P1 |
| `src/features/projects/ProjectsPage.tsx:82` | "Import" | Project import flow | P2 |
| `src/features/targets/TargetsPage.tsx:88` | "Import OpenAPI" | OpenAPI import → targets | P2 |
| `src/features/discovery/DiscoveryPage.tsx:20,59,61,64` | "Configure Modules", "Pause", "View Results", "Run Now" | Discovery config + job control + results view | P2 |
| `src/features/attacks/AttacksPage.tsx:77,78,90` | "Playbook", "Launch Attack", "Configure" (×9) | Attack config/launch UI | P2 |
| `src/features/findings/FindingsPage.tsx:88,89` | "Export SARIF", "Triage" | SARIF export + triage workflow | P2 |
| `src/features/reports/ReportsPage.tsx:69,83` | "Download", "Templates" | Report download + template picker | P2 |
| `src/features/models/ModelsPage.tsx:20,21,54,55,59,62` | "Browse HuggingFace", "Download Model", "Verify", "Remove", "Download", "Cancel" | Wire to `aisec-models` via IPC | P2 (optional module) |
| `src/features/settings/SettingsPage.tsx:124` | "View Logs" | Open log directory/file | P2 |
| `src/features/findings/FindingsPage.tsx` | `UPDATE_FINDING_STATUS` (in-memory) | Persist status via IPC | P2 |
| `src/features/settings/SettingsPage.tsx` | `UPDATE_SETTING` (in-memory) | Persist settings (config file/DB) | P2 |

---

## C. Tauri backend skeleton — **P0** (MVP blockers B1, B2)

| File | Function | Real implementation required | Priority |
|------|----------|------------------------------|----------|
| `src-tauri/src/commands/mod.rs` | `health`, `app_info` (only commands) | Add domain commands: `project_create`, `project_list`, `target_create`, `scan_run`, `findings_list`, `report_generate` | **P0** |
| `src-tauri/src/state.rs` | `AppState` (holds only `_log_guard`) | Add a `Database` handle (open SQLite via `aisec-storage` on startup) | **P0** |
| `src-tauri/Cargo.toml` | dependencies (only `aisec-core`) | Add `aisec-storage`, `aisec-discovery`, `aisec-attack`, `aisec-report` (+ transitive `aisec-payload`); add a thin `scan_run` orchestrator | **P0** |

---

## D. Production `let _ =` discards & hardcoded fallbacks — **P1 / P2**

These silently drop a parameter or substitute a hardcoded value in non-test code.

| File | Function | Real implementation required | Priority |
|------|----------|------------------------------|----------|
| `crates/aisec-core/src/logging.rs:66` | `init_logging` — `let _ = options.json_file;` | Honor the `json_file` flag (emit JSON-formatted file logs) or remove the flag | P2 |
| `crates/aisec-auth/src/engine.rs:255` | `authenticate_api_key` — `let _ = header_name;` | Persist/apply the custom header name for downstream HTTP auth | P2 (auth is optional for MVP) |
| `crates/aisec-attack/src/orchestrator.rs:97` | `run` — `let _ = idx;` | Implement real concurrency using `OrchestratorConfig.concurrency` (currently sequential) | P2 |
| `crates/aisec-fingerprint/src/scoring.rs` | `suggest_method()` returns hardcoded `Some("POST")` | Derive method from matched signals/provider | P2 (fingerprint optional) |
| `crates/aisec-models/src/hardware/detect.rs:79` | RAM fallback hardcodes 8 GiB on non-Linux/macOS | Real detection for other platforms or surface "unknown" | P2 |
| `crates/aisec-models/src/types.rs:134` | `recommended_gpu_layers()` returns flat 35/0 | VRAM-aware layer count | P2 |
| `crates/aisec-discovery/src/engine.rs` | `ProbeOutput.errors` always empty | Record probe failures so the UI/report can show coverage gaps | P1 |

---

## E. Domain behavioral placeholders (dead config & shallow logic) — **P1 / P2**

Fields/branches that exist but are unused or implemented shallowly (the "looks done, does nothing"
class).

| File | Function / symbol | Real implementation required | Priority |
|------|-------------------|------------------------------|----------|
| `crates/aisec-discovery/src/crawler.rs` | worker pool (`notify_one`/`in_flight` race) | Fix multi-worker deadlock so `worker_count > 1` works (MVP runs `1` as workaround) | **P1** (B4) |
| `crates/aisec-discovery/src/detectors/ai.rs` | `probe_ai_paths` skips POST on GET 404 | POST when GET is 404/405 so `/v1/chat/completions` is found | **P1** (B6) |
| `crates/aisec-discovery/src/url_policy.rs` | SSRF guard | DNS resolution + redirect re-validation (hostname/literal-IP only today) | P2 |
| `crates/aisec-attack/src/types.rs` | `AttackBudget.max_mutations_per_payload` (unused) | Enforce in mutator (uses hardcoded `max_per_payload: 3`) | P1 |
| `crates/aisec-attack/src/types.rs` | `TargetKind`, `PayloadFormat::MultiTurn` (unused) | Route by target kind; support multi-turn attacks | P2 |
| `crates/aisec-attack/src/error.rs` | `BudgetExhausted`, `Cancelled` (never constructed) | Wire budget/cancellation paths | P2 |
| `crates/aisec-attack/src/traits.rs` | `supported_mutators()` (never called) | Use it to gate mutator selection per attack | P2 |
| `crates/aisec-report/src/charts.rs:64` & `formatters/pdf.rs` | PDF "simplified as stacked bars"; single physical page | Real PDF pagination + risk-gauge/category charts | P2 (PDF not in MVP) |
| `crates/aisec-plugin-host/src/permissions.rs` & `sandbox/runner.rs` | `check_path_read` never called; `max_output_bytes` unused; env stripping = 2 keys; `$PLUGIN_DIR` not expanded; host calls audit-only | Real permission enforcement + output cap + env hygiene + path expansion | P2 (plugin host not in MVP) |

---

## F. Test-only mock runtimes / drivers / transports — **P3 (keep)**

Legitimate test doubles. The **real** implementation already exists and is the production default;
these are not used on production paths. Keep them; do not "remove."

| File | Symbol | Real implementation (already exists) | Priority |
|------|--------|--------------------------------------|----------|
| `crates/aisec-attack/src/transport/mock.rs` | `MockTransport` (always returns response index 0) | `transport/http.rs` `HttpTransport` (production default) | P3 keep |
| `crates/aisec-models/src/runtime/mock.rs` | `MockInferenceRuntime` | `runtime/llama_cpp.rs` `LlamaCppRuntime` (production default) | P3 keep |
| `crates/aisec-auth/src/mock.rs` | `MockPlaywrightDriver` | `playwright/client.rs` `PlaywrightClient` (production default) | P3 keep |
| `crates/aisec-judge/src/mock_runtime.rs` | `JsonMockRuntime` | Any real `InferenceRuntime` (e.g. `LlamaCppRuntime`) | P3 keep |

> Note: these are `pub`-exported from their crates. That is acceptable for test harnesses across the
> workspace, but consider gating behind a `#[cfg(any(test, feature = "test-util"))]` so they cannot
> be wired into production by mistake. (Optional hardening, not MVP.)

---

## G. Test fixtures (`stub` / `fake` strings) — **P3 (keep)**

Hardcoded bytes/paths inside `#[cfg(test)]` code. Not production placeholders.

| File | Location | Note | Priority |
|------|----------|------|----------|
| `crates/aisec-models/tests/integration.rs:15` | `b"GGUF-stub-model-bytes-for-test"` | Test GGUF bytes | P3 keep |
| `crates/aisec-models/tests/integration.rs:59` | `/tmp/fake.gguf` | Test path | P3 keep |
| `crates/aisec-models/src/registry.rs:132` | `b"gguf-stub"` (in `#[cfg(test)]`) | Test fixture | P3 keep |
| `crates/aisec-plugin-host/src/manager.rs:276` | `"print('stub')"` (in `#[cfg(test)]`) | Test plugin body | P3 keep |

---

## H. Plugin SDK abstract stubs (by design) — **P3**

These are abstract base methods meant to be overridden by plugin authors; raising "not implemented"
is the intended contract, not a defect.

| File | Function | Real implementation required | Priority |
|------|----------|------------------------------|----------|
| `packages/plugin-sdk-python/aisec_plugin/discovery.py:13` | `discover` → `NotImplementedError` | Override by plugin authors (keep abstract) | P3 |
| `packages/plugin-sdk-python/aisec_plugin/attack.py:13` | `executeAttack` → `NotImplementedError` | Override by plugin authors | P3 |
| `packages/plugin-sdk-python/aisec_plugin/judge.py:13` | `evaluate` → `NotImplementedError` | Override by plugin authors | P3 |
| `packages/plugin-sdk-python/aisec_plugin/report.py:13` | `renderReport` → `NotImplementedError` | Override by plugin authors | P3 |
| `packages/plugin-sdk-js/src/discovery.js:9` | `discover()` throws "not implemented" | Override by plugin authors | P3 |
| `packages/plugin-sdk-js/src/attack.js:9` | `executeAttack()` throws | Override by plugin authors | P3 |
| `packages/plugin-sdk-js/src/judge.js:9` | `evaluate()` throws | Override by plugin authors | P3 |
| `packages/plugin-sdk-js/src/report.js:9` | `renderReport()` throws | Override by plugin authors | P3 |

> Gap (tracked elsewhere): SDK `PluginContext` lacks documented helpers (`http_request`, filesystem,
> `probe_mutate`). Implement when the plugin story is in scope.

---

## I. Sample plugin hardcoded data — **P3**

| File | Function | Real implementation required | Priority |
|------|----------|------------------------------|----------|
| `plugins/samples/discovery-openapi-paths/plugin.py` | `discover()` returns hardcoded `COMMON_PATHS` | Real probing via host `DiscoveryEngine` once plugin host is wired (also fix the sample manifest `[permissions.rationale]` schema break) | P3 |

---

## J. TODO markers — **P1**

| File | Function | Real implementation required | Priority |
|------|----------|------------------------------|----------|
| `crates/aisec-discovery/examples/verify_target.rs:15` | `main` — `// TODO: use 4+ after crawler deadlock fix` | Restore `worker_count >= 4` after fixing the crawler deadlock (see Category E) | P1 |

**`unimplemented!()` / `todo!()` macros:** none found anywhere in the repository.

---

## Removal sequencing (to reach a real MVP scan)

1. **P0 — backend spine (Category C):** add domain crate deps, open SQLite in `AppState`, implement the 6 IPC commands + a thin `scan_run` orchestrator.
2. **P0 — un-mock the UI (Categories A, B):** seed the store from IPC, wire the 4 MVP buttons (New Project, Add Target, Start Scan, Generate Report), delete/gate `mock/data.ts`.
3. **P1 — discovery correctness (Category E/J):** run `worker_count: 1` + `allow_private_network` now; fix the crawler deadlock and AI POST probe to remove the workarounds; record probe errors.
4. **P1 — attack/report fidelity (Categories D, E):** enforce attack budget, populate report inputs from stored findings.
5. **P2 — shipped-crate polish:** `json_file` logging, fingerprint method, models GPU tuning, PDF charts, plugin-host enforcement.
6. **P3 — leave as-is:** test doubles, test fixtures, SDK abstract bases (revisit only when those subsystems enter scope).

**Bottom line:** removing mocks to get a real scan is dominated by **P0 integration work** (backend
commands + un-mocking the React store), not by replacing scattered stub macros — there are none. The
domain crates already contain the real implementations the UI should call.
