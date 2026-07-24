# PromptLab — Tech Lead Review

> Staff Engineer assessment of the PromptLab desktop codebase (v0.1.0).  
> Review date: 2026-06-13 · Scope: architecture, maintainability, scalability, performance, security, technical debt  
> **No code was modified** as part of this review.

---

## Executive summary

PromptLab is a **well-structured late-alpha product** with a clear separation between React UI, Tauri IPC, and a Rust workspace of domain engines. The primary scan workflow (project → target → discovery → attack → judge → findings → report) is **real and wired**, not a mock prototype. Library-layer quality is ahead of product-layer integration: several crates (`promptlab-models`, `promptlab-plugin-host`, `promptlab-fingerprint`) are built but not connected to the desktop shell.

**Strengths:** Crate boundaries, testable `*_op` command pattern, typed IPC DTOs, SQLite repository layer, embedded payload library, deterministic judge in the attack path, Playwright auth bundling for release.

**Primary risks before MVP ship:** (1) credentials stored in plaintext in SQLite, (2) discovery SSRF guard permanently bypassed in IPC, (3) auth sessions not replayed in attacks, (4) workspace-wide frontend refresh pattern, (5) CI test suite not green, (6) contract gaps (`disabled_tests`, dashboard placeholders) that erode operator trust.

**Overall recommendation:** Ship MVP only after closing **High** security and contract-integrity items. Medium items can be scheduled in the next sprint; Low/Info items are acceptable backlog for a 0.1.x desktop tool used by authorized testers.

---

## Review methodology

Assessment based on:

- Source review of `src-tauri/`, `src/`, and `crates/`
- Existing audit docs (`PROJECT_CURRENT_STATE.md`, `ARCHITECTURE_DIAGRAM.md`, `DISCOVERY_VERIFICATION_REPORT.md`)
- AGENTS.md test/build status
- Tauri capabilities, error handling, and data flow patterns

Severity scale:

| Severity | Meaning |
|----------|---------|
| **Critical** | Blocks safe production use or causes data loss/corruption |
| **High** | Significant risk to security, correctness, or release confidence |
| **Medium** | Meaningful impact on ops, velocity, or scale; workaround exists |
| **Low** | Minor issue; fix when convenient |
| **Info** | Observation or positive note; no immediate action |

---

## Architecture

### Finding A1 — Strong crate decomposition and IPC layering

| | |
|---|---|
| **Severity** | Info |
| **Explanation** | Business logic lives in focused crates (`promptlab-discovery`, `promptlab-attack`, `promptlab-judge`, `promptlab-report`, `promptlab-storage`). Tauri commands are thin wrappers over testable `*_op` functions (`commands/domain.rs`, `commands/scan.rs`). DTO mapping is centralized in `src-tauri/src/dto.rs`. This is the right shape for a security tool that will evolve independently per engine. |
| **Recommendation** | Preserve this pattern. New features should extend crates first, then add IPC — not embed logic in `src-tauri`. Document the rule in CONTRIBUTING (one page). |

---

### Finding A2 — Product integration lags library capability

| | |
|---|---|
| **Severity** | High |
| **Explanation** | The workspace exposes 12 crates but the desktop app only exercises a subset. `promptlab-models`, `promptlab-plugin-host`, and `promptlab-fingerprint` are not wired to IPC or UI. The judge LLM path exists in `promptlab-judge` but production attack flow calls `judge_deterministic()` only. Playwright auth persists sessions to SQLite/vault but `session_id` is not written into `targets.descriptor_json`, so browser auth does not flow to `apply_descriptor_auth` in attacks. Operators see UI affordances (Models page, plugin samples) that imply capabilities the app does not deliver. |
| **Recommendation** | Publish an explicit **integration matrix** (crate × IPC × UI) and treat unintegrated crates as "library-only" in UX until wired. Prioritize auth session → attack transport as the highest integration gap on the critical path. Defer Models/plugins UI or gate behind "Coming soon" to avoid false expectations. |

---

### Finding A3 — Mixed sync/async execution models

| | |
|---|---|
| **Severity** | Medium |
| **Explanation** | `scan_start` correctly uses a background tokio task + `ScanJobManager`. `discovery_run` and `report_generate` run **synchronously inside the IPC handler**, blocking until HTTP crawl or file generation completes. Architecturally this splits the product into two execution paradigms without a unified job abstraction. Long discovery runs tie up the invoke channel; UI can only show a spinner, not cancel mid-flight. |
| **Recommendation** | Introduce a unified `JobManager` trait covering discovery, scan, and report jobs with shared progress/cancel semantics. Short term: document sync commands in API docs and add frontend timeouts/messaging. Medium term: move `discovery_run` to background task parity with `scan_start`. |

---

### Finding A4 — Dual persistence for scan wizard

| | |
|---|---|
| **Severity** | Medium |
| **Explanation** | Wizard draft state lives in `sessionStorage` (`wizardState.ts`, key `promptlab:scan-wizard` v2) while committed entities live in SQLite. Steps 2–4 can diverge from DB if the user refreshes mid-flow or if target persist fails silently. `submittedScanId` links wizard step 6 to DB only after successful `scan_start`. |
| **Recommendation** | Treat sessionStorage as a cache, not source of truth. On wizard mount, reconcile against DB (target fingerprint vs `savedTargetFingerprint`). Consider server-side draft scans or explicit "Save draft" that writes partial playbook to SQLite. |

---

### Finding A5 — Unregistered `db_health` command

| | |
|---|---|
| **Severity** | Low |
| **Explanation** | `db_health` is implemented in `commands/mod.rs` but omitted from `lib.rs` `invoke_handler`. Dead code or incomplete API surface creates confusion for integrators and tests. |
| **Recommendation** | Either register it for diagnostics/settings page or remove it. If registered, use for health dashboard instead of duplicating logic in `health`. |

---

## Maintainability

### Finding M1 — Workspace load pattern couples all features to full refresh

| | |
|---|---|
| **Severity** | Medium |
| **Explanation** | `AppStore.loadAll()` (`AppStore.tsx`) loads projects, then per-project targets/scans, then all findings, then **per-scan endpoints**. Every mutation calls `refresh()`, reloading the entire workspace. Any feature change risks breaking unrelated pages. Mapper logic in `mappers.ts` aggregates counts client-side. |
| **Recommendation** | Split store into domain slices with targeted invalidation (e.g. `refreshFindings()`, `refreshScans(projectId)`). Add a backend `workspace_snapshot` command if round-trips remain high. Keep mappers pure and unit-tested. |

---

### Finding M2 — IPC/client contract drift

| | |
|---|---|
| **Severity** | High |
| **Explanation** | `scan_start` accepts `disabled_tests` and stores them in `playbook_json`, but `run_scan_job` never passes them to `run_category_on_endpoint`. The attack plan UI lets operators disable individual tests; the backend ignores the selection. This is a **behavioral contract bug**, not cosmetic debt — it will surface in security assessments as unreliable tooling. |
| **Recommendation** | Filter payloads in `PayloadRunner` or skip categories/tests in `run_scan_job` based on `disabled_tests`. Add an integration test asserting a disabled payload ID never produces an `attack_result` row. |

---

### Finding M3 — Documentation sprawl and staleness

| | |
|---|---|
| **Severity** | Medium |
| **Explanation** | `docs/` contains 30+ files including overlapping audits (`MVP_*`, `MOCK_*`, `REAL_IMPLEMENTATION_*`, `STATUS.md`, `PROJECT_STRUCTURE.md`). `PROJECT_STRUCTURE.md` still describes "bootstrap only" IPC (health + app_info). `AGENTS.md` claims browser mode "falls back to mock data" — mock fixtures were removed; behavior is empty state. Stale docs increase onboarding cost and wrong operational decisions. |
| **Recommendation** | Consolidate to a **canonical set**: `PROJECT_CURRENT_STATE.md`, `ARCHITECTURE_DIAGRAM.md`, `AUTH.md`, `DATABASE.md`, engine-specific docs. Archive or delete superseded audits. Add "last verified" dates. Fix AGENTS.md mock mode description. |

---

### Finding M4 — Monolithic frontend styling

| | |
|---|---|
| **Severity** | Low |
| **Explanation** | `src/styles/global.css` is a large single file (~2.6k+ lines per UX report). Component styles are not co-located. Refactors to shared components risk unintended cross-page regressions. |
| **Recommendation** | Incrementally extract feature-scoped CSS modules or CSS layers (`@layer components`, `@layer pages`). Not blocking MVP. |

---

### Finding M5 — Test suite does not gate releases

| | |
|---|---|
| **Severity** | High |
| **Explanation** | Per `AGENTS.md`, `cargo test --workspace` fails due to multiple pre-existing issues (storage test API drift, `promptlab-auth` tokio feature, integration-tests missing `tracing`, discovery hang, judge/plugin-host failures). Frontend tests pass (`npm test`). A Staff review cannot sign off on release discipline without a green CI bar. |
| **Recommendation** | Fix failing tests or quarantine network-dependent tests behind `#[ignore]`. Add CI workflow: `npm test`, `cargo test --workspace`, `cargo build --workspace`. Make PR merge depend on green CI. |

---

### Finding M6 — Typed boundaries are a maintainability win

| | |
|---|---|
| **Severity** | Info |
| **Explanation** | TypeScript IPC wrappers mirror Rust DTOs (`shared/ipc/client.ts`, `dto.rs`). `CommandError` maps to `toAppError()` consistently. `targetDescriptor.test.ts` and other Vitest tests cover critical wizard logic. |
| **Recommendation** | Extend typed coverage to `scanPlaybook.ts` parsing and report download paths. Consider generating TS types from Rust schemas in a future iteration. |

---

## Scalability

### Finding S1 — Frontend data model does not scale with workspace size

| | |
|---|---|
| **Severity** | Medium |
| **Explanation** | Loading all findings and all endpoints for all scans works for demo-scale data but degrades linearly with projects/scans/endpoints. No pagination at IPC layer for findings or endpoints. `FindingsPage` filters client-side only. |
| **Recommendation** | Add paginated IPC (`finding_list_page`, `endpoint_list_page`) before customers run repeated weekly scans. SQLite FTS on findings is already present — expose search server-side. |

---

### Finding S2 — Scan throughput is intentionally sequential

| | |
|---|---|
| **Severity** | Medium |
| **Explanation** | `run_scan_job` iterates endpoints × categories serially. Appropriate for rate-limit safety on target APIs, but limits throughput on large endpoint sets (e.g. 50 endpoints × 9 categories = 450 sequential HTTP+judge units). No concurrency knob. |
| **Recommendation** | Add configurable `max_parallel_probes` with per-target rate limiting. Default to 1 for safety; allow operators to raise for internal lab targets. Persist queue position in playbook progress. |

---

### Finding S3 — Discovery crawl capped and single-threaded

| | |
|---|---|
| **Severity** | Medium |
| **Explanation** | IPC hardcodes `max_depth: 2`, `max_pages: 25`, `worker_count: 1` due to crawler deadlock at higher worker counts (`DISCOVERY_VERIFICATION_REPORT.md`). Discovery will not scale to large SPAs or deep sites without fixing root cause. |
| **Recommendation** | Fix crawler concurrency bug, add integration test with `worker_count: 2`. Expose discovery limits in UI as advanced settings rather than hardcoding in `discovery.rs`. |

---

### Finding S4 — SQLite appropriate for desktop MVP

| | |
|---|---|
| **Severity** | Info |
| **Explanation** | Single-user desktop app with WAL mode (`pool.rs`, `max_connections(5)`) is a sound choice. No need for client-server DB at current scale. |
| **Recommendation** | Document backup/export path for `promptlab.db`. Consider periodic VACUUM guidance in ops docs. |

---

## Performance

### Finding P1 — Synchronous discovery blocks IPC thread

| | |
|---|---|
| **Severity** | High |
| **Explanation** | `discovery_run_op` awaits full `DiscoveryEngine::discover()` before returning. Large targets can stall the Tauri async runtime thread pool entry for tens of seconds. UI thread remains responsive, but concurrent IPC calls may queue behind discovery. |
| **Recommendation** | Background discovery job with progress events (pages fetched / endpoints found). Return `scan_id` immediately like `scan_start`. |

---

### Finding P2 — Full workspace refresh on every mutation

| | |
|---|---|
| **Severity** | Medium |
| **Explanation** | `runMutation` and most actions call `refresh()` → O(projects + scans) IPC invocations. Creating one target reloads all findings and all endpoints. Noticeable latency as data grows. |
| **Recommendation** | See M1. Optimistic UI updates for create/delete with targeted refetch. |

---

### Finding P3 — Scan status polling every 2 seconds

| | |
|---|---|
| **Severity** | Low |
| **Explanation** | `useScanStatuses.ts` polls `getScanStatus` per scan ID every `POLL_MS = 2000`. Acceptable for one active scan; wasteful if extended to dashboard monitoring many jobs. |
| **Recommendation** | Replace with Tauri events (`scan://progress`) when background job emits progress. Exponential backoff when scan is idle/completed. |

---

### Finding P4 — HTTP response bodies loaded fully into memory

| | |
|---|---|
| **Severity** | Low |
| **Explanation** | `HttpTransport` reads full response text (`transport/http.rs`). LLM responses can be large; stored in `attack_results.response_json` and finding evidence excerpts (500 chars). Memory and DB bloat possible on verbose targets. |
| **Recommendation** | Cap stored body size at persistence layer. Truncate before judge evaluation if over threshold. |

---

## Security

### Finding SEC1 — Credentials stored in plaintext in SQLite

| | |
|---|---|
| **Severity** | Critical |
| **Explanation** | Target descriptors persist passwords, API keys, and JWT tokens in `targets.descriptor_json` (`targetDescriptor.ts` → `target_create`). SQLite file at `{app_data}/promptlab.db` is unencrypted. Evidence and attack results may also contain sensitive response bodies. This is a **local secrets exposure** risk: backup tools, malware, shared machines, or accidental log upload can leak credentials. |
| **Recommendation** | Use OS keychain (macOS Keychain, Windows DPAPI, Linux secret-service) for secrets; store references in descriptor. Minimum interim: encrypt sensitive fields with a machine-bound key. Never log descriptor contents. Document threat model: "single-operator trusted workstation." |

---

### Finding SEC2 — SSRF guard bypass hardcoded in discovery IPC

| | |
|---|---|
| **Severity** | High |
| **Explanation** | `discovery_run_op` sets `allow_private_network: true` unconditionally (`discovery.rs`). The engine's default SSRF policy (`url_policy.rs`, `allow_private_network: false`) is overridden for every run. Any seed URL pointing at internal IPs (169.254.x, RFC1918, localhost) is crawled. For a pentest tool this is intentional for lab use, but **without UI opt-in or scope confirmation** it increases accidental internal scanning risk. |
| **Recommendation** | Default to `false` for external targets; require explicit operator toggle "Allow private/internal networks" in wizard with warning. Log all discovery targets to audit trail. Consider target allowlist per project. |

---

### Finding SEC3 — Attack phase sends real offensive payloads over HTTP

| | |
|---|---|
| **Severity** | Info (by design) |
| **Explanation** | `promptlab-payload/data/payloads.json` contains active probe strings (injection, jailbreak, exfil instructions). `HttpTransport` executes against operator-selected endpoints. This is correct product behavior for authorized testing. |
| **Recommendation** | Add scan authorization acknowledgement in UI (scope checkbox, target URL display). Include rate limiting defaults. Document legal/authorization requirements in product README. |

---

### Finding SEC4 — Playwright auth sessions not applied to attacks

| | |
|---|---|
| **Severity** | Medium |
| **Explanation** | Interactive login saves `storage_state_path` to auth vault and returns `sessionId`, but target descriptor does not reference the session. `apply_descriptor_auth` supports token headers from descriptor but not cookie replay from Playwright storage. Operators may believe authenticated scanning works when attacks run unauthenticated. |
| **Recommendation** | Link `session_id` in descriptor; extend attack transport to inject cookies from vault or use Playwright for authenticated probes. Until fixed, UI must warn that User/Pass and SSO auth do not affect HTTP attacks. |

---

### Finding SEC5 — Minimal Tauri capability surface

| | |
|---|---|
| **Severity** | Info |
| **Explanation** | `capabilities/default.json` grants only core window/path defaults — no broad filesystem or shell permissions. Reduces desktop attack surface from malicious frontend code. |
| **Recommendation** | Review capabilities before adding file pickers or shell open. Use scoped allowlists per command. |

---

### Finding SEC6 — Plugin sandbox exists but is not production-hardened for integration

| | |
|---|---|
| **Severity** | Medium |
| **Explanation** | `promptlab-plugin-host` runs Python/Node subprocesses with JSON-lines protocol and `PermissionGuard`, but plugins are not loaded in the desktop app. When integrated, subprocess plugins remain a **supply chain risk** if operators install third-party plugins. |
| **Recommendation** | Before enabling plugins: signature verification, permission prompts, network/file capability defaults deny, plugin review docs. Treat as post-MVP with security review gate. |

---

### Finding SEC7 — SQL access uses parameterized repositories

| | |
|---|---|
| **Severity** | Info |
| **Explanation** | `promptlab-storage` uses `sqlx` with bound parameters in repository implementations. No string-concatenated SQL observed in command layer. Foreign keys enabled at connect. |
| **Recommendation** | Maintain repository-only DB access rule. Add sqlx compile-time check where feasible. |

---

## Technical Debt

### Finding TD1 — Crawler multi-worker deadlock

| | |
|---|---|
| **Severity** | High |
| **Explanation** | Documented in `DISCOVERY_VERIFICATION_REPORT.md` and enforced via `worker_count: 1` in IPC. Root cause unfixed; blocks performance and invalidates default `DiscoveryConfig`. |
| **Recommendation** | Dedicated engineering task: reproduce with minimal test, fix shared-state deadlock, restore default worker_count > 1. |

---

### Finding TD2 — Duplicate Playwright runtimes

| | |
|---|---|
| **Severity** | Medium |
| **Explanation** | Auth bundling (`scripts/bundle-playwright-auth.sh`, `src-tauri/resources/playwright/`) and discovery Playwright (`crates/promptlab-discovery/playwright/`) are separate Node trees. Release size and maintenance burden doubled. |
| **Recommendation** | Unify on one bundled runtime path referenced by both crates via env vars (`PLAYWRIGHT_BROWSERS_PATH`, shared runner). |

---

### Finding TD3 — Dashboard and Models UI placeholders

| | |
|---|---|
| **Severity** | Medium |
| **Explanation** | `DashboardPage` uses hardcoded stat hints ("2 active", "1 scanning"); `activity`, `discoveryJobs`, `attackRuns` arrays never populated. `ModelsPage` renders empty list with non-functional buttons. Erodes trust in a security product where accuracy matters. |
| **Recommendation** | Remove false hints; derive stats from `AppStore`. Hide Models nav until IPC exists or show explicit empty state with roadmap link. |

---

### Finding TD4 — Incomplete CRUD on targets and findings

| | |
|---|---|
| **Severity** | Medium |
| **Explanation** | No `target_update` / `target_delete` IPC. Finding status changes are frontend-only (`UPDATE_FINDING_STATUS` reducer) and not persisted. Operators cannot close findings or edit targets without workarounds. |
| **Recommendation** | Add `target_update`, `target_delete`, `finding_update` commands with repository support. Align UI actions with backend. |

---

### Finding TD5 — Toolchain metadata drift

| | |
|---|---|
| **Severity** | Low |
| **Explanation** | Workspace `rust-version = "1.77"` in root `Cargo.toml` contradicts AGENTS.md requirement for stable ≥ 1.85 due to transitive `edition2024` deps. New contributors may use wrong toolchain. |
| **Recommendation** | Update `rust-version` and add `rust-toolchain.toml` pinning stable. |

---

### Finding TD6 — Judge LLM path built but unused in production

| | |
|---|---|
| **Severity** | Medium |
| **Explanation** | `promptlab-judge` supports multi-model consensus; attack path uses `judge_deterministic()` only. Settings include `autoJudge: true` but it has no effect. False negatives/positives likely on nuanced LLM responses with regex-only judging. |
| **Recommendation** | Wire `ModelRolePool` when GGUF registered, or disable `autoJudge` in settings UI until implemented. Document deterministic judge limitations in report footers. |

---

### Finding TD7 — Positive: real E2E spine over mock-driven UI

| | |
|---|---|
| **Severity** | Info |
| **Explanation** | Mock fixture layer removed. `mvp_flow.rs` integration test exercises real storage and engines. Attack findings originate from HTTP + judge, not hardcoded UI data. This is above average for early desktop security tools. |
| **Recommendation** | Protect this with CI and expand `mvp_flow` to cover wizard-equivalent sequence including report export. |

---

## Priority matrix

| ID | Area | Severity | Effort (est.) | Suggested sprint |
|----|------|----------|---------------|------------------|
| SEC1 | Security | Critical | L | Immediate |
| SEC2 | Security | High | S | Immediate |
| M2 | Maintainability | High | S | Immediate |
| M5 | Maintainability | High | M | Sprint 1 |
| TD1 | Tech debt | High | M | Sprint 1 |
| A2 | Architecture | High | L | Sprint 1–2 |
| P1 | Performance | High | M | Sprint 2 |
| SEC4 | Security | Medium | M | Sprint 2 |
| M1/P2 | Performance | Medium | M | Sprint 2 |
| TD4 | Tech debt | Medium | S | Sprint 2 |
| TD6 | Tech debt | Medium | L | Sprint 3 |
| S1 | Scalability | Medium | M | Post-MVP |

*Effort: S = small (1–3 days), M = medium (1–2 weeks), L = large (2+ weeks)*

---

## Release readiness verdict

| Criterion | Status |
|-----------|--------|
| Core scan workflow functional | ✅ Yes (with Tauri backend) |
| Data integrity / contract honesty | ⚠️ `disabled_tests` gap, auth session gap |
| Security posture for desktop pentest tool | ❌ Plaintext secrets; SSRF bypass always on |
| CI / test confidence | ❌ Rust workspace tests not fully green |
| Documentation accuracy | ⚠️ Stale docs remain |
| Scalability for repeated production use | ⚠️ Acceptable for MVP; not for heavy users |

**Verdict:** **Not ready for external beta** until SEC1, SEC2, and M2 are resolved and CI is green. **Ready for internal authorized testing** with documented limitations if credentials are treated as disposable lab values only.

---

## Suggested 30-day engineering focus

1. **Secrets handling** — Keychain-backed credential storage; stop writing raw passwords/JWTs to SQLite.
2. **Contract integrity** — Enforce `disabled_tests`; link Playwright `sessionId` to descriptor or disable UI claim.
3. **CI bar** — Fix or ignore-flake Rust tests; mandatory `npm test` + `cargo test` on PR.
4. **Discovery jobs** — Background discovery + operator-controlled `allow_private_network`.
5. **Doc consolidation** — Single source of truth; archive redundant audits.
6. **Target/finding CRUD** — Close basic persistence gaps operators expect from a DB-backed app.

---

*Review conducted without code changes. Cross-reference: [PROJECT_CURRENT_STATE.md](./PROJECT_CURRENT_STATE.md) · [ARCHITECTURE_DIAGRAM.md](./ARCHITECTURE_DIAGRAM.md)*
