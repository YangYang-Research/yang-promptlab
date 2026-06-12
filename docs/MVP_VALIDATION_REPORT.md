# AISec MVP Validation Report

**Date:** 2026-06-11
**Scope:** Execute and validate the complete MVP scan flow end-to-end; identify every runtime
failure; fix only blockers preventing the flow.
**Method:** A real integration harness (`tests/integration/tests/mvp_flow.rs`) drives all seven
steps against a **live local HTTP target** (wiremock) using the production crate APIs — real HTTP,
real SQLite, real evaluation, real HTML. No mocks on the flow path.

> This validation runs on an integration branch that combines the per-module fixes from the prior
> PRs (#5 discovery, #8 fingerprint, #9 attack scanner, #10 models, #11 judge, #12 report). On
> `main` (none merged) the flow fails at multiple points; see §3.

---

## 1. Result: MVP flow PASSES end-to-end

```
[1] project created: 019eb7fc-…                         (aisec-storage)
[2] target added: …  http://127.0.0.1:39497             (aisec-storage)
[3] discovery: pages=3 probes=32 endpoints=4 errors=0   (aisec-discovery, worker_count=8)
[4] AI endpoints discovered: 2                           (aisec-discovery + aisec-fingerprint)
      - POST /v1/chat/completions (conf 0.90)
      - GET  /v1/models (conf 0.90)
    fingerprint primary: openai
[5] attack: payloads_sent=12 findings_stored=12 highest=Critical   (aisec-attack, real HTTP)
[6] judge verdict: vulnerable=true confidence=0.97 severity=Critical  (aisec-judge, rule+regex)
[7] report written: reports/aisec-technical-….html (16687 bytes)     (aisec-report)
MVP end-to-end flow PASSED
```

Every step executed with no runtime errors. Notable: discovery ran with the default 8 workers
(no deadlock), and `/v1/chat/completions` (POST-only, GET→404) was found via the AI POST probe.

### Step-by-step

| # | Step | Module(s) | Outcome |
|---|------|-----------|---------|
| 1 | Create project | `aisec-storage` | Project row created in SQLite |
| 2 | Add target URL | `aisec-storage` | Target row created (FK to project) |
| 3 | Run Discovery | `aisec-discovery` | 3 pages crawled, 32 static probes, 4 endpoints, 0 errors |
| 4 | Detect AI endpoints | `aisec-discovery` + `aisec-fingerprint` | 2 AI endpoints; provider fingerprinted as `openai` |
| 5 | Prompt injection attack | `aisec-attack` | 12 payloads sent over real HTTP; 12 findings persisted; severity Critical |
| 6 | Evaluate response | `aisec-judge` | `vulnerable=true`, confidence 0.97, Critical |
| 7 | Generate report | `aisec-report` | HTML written to `reports/` with project/target/payload/response/severity/confidence |

---

## 2. Runtime failures found & blockers fixed (this task)

The flow harness could not even be built/run until two blockers were fixed. Both are bug/wiring
fixes (no new product features).

| # | Failure | Where | Fix |
|---|---------|-------|-----|
| B1 | **Integration crate did not compile** — `core_smoke.rs` calls `tracing::info!` but `tracing` was not a dependency; the crate also lacked the MVP module deps needed to host the harness. | `tests/integration/Cargo.toml` | Added `tracing` plus the MVP crate deps (`aisec-discovery`, `aisec-fingerprint`, `aisec-attack` with `storage`, `aisec-judge`, `aisec-report`, `serde_json`, `wiremock`, tokio `macros`/`rt-multi-thread`). |
| B2 | **`aisec-storage` unit tests fail to compile** (`E0599: no method named create`) — two test modules call `repos.scans().create(...)` without importing the `ScanRepository` trait. A required MVP module was left non-testable. | `repositories/sqlite/finding.rs`, `repositories/sqlite/attack_result.rs` | Added `ScanRepository` to the test `use`. (Production storage code was already correct — the MVP flow uses it successfully.) |

No other runtime failures occurred on the MVP flow path.

---

## 3. Blockers that previously prevented the MVP (resolved by prior PRs)

These were genuine runtime failures that would break the flow; the validation confirms they are now
resolved (they are the substance of the integrated PRs, not re-fixed here):

| Blocker | Step affected | Resolution (PR) |
|---------|---------------|-----------------|
| Crawler deadlocks with `worker_count > 1` (default 8) → discovery hangs | 3 | Worker-pool rewrite (#5) — validated here with 8 workers, no hang |
| AI POST-only endpoints missed when GET returns 404 (`/v1/chat/completions`) | 4 | POST-on-no-GET-detection fix (#5) — endpoint found here |
| `aisec-attack` test suite fails to compile (`E0308` in `payload/runner.rs`) | 5 | Borrow fix (#9); scanner + storage sink added (#9) |
| Judge consensus misses spaced `API key:`; refusal false-positive | 6 | Regex + refusal fixes (#11) |
| Reports lacked payload/response/confidence fields | 7 | `ReportFinding` extended + HTML rendering (#12) |

---

## 4. Per-crate test status (MVP path)

| Crate | `cargo test` | Notes |
|-------|--------------|-------|
| `aisec-storage` | ✅ 11 passed | after B2 fix |
| `aisec-discovery` | ✅ 27 + 3 passed | multi-worker crawl + AI POST covered |
| `aisec-fingerprint` | ✅ 23 passed | 8 providers + OpenAPI analysis |
| `aisec-attack` (`--features storage`) | ✅ 12 + 2 + 2 passed | scanner integration via real HTTP |
| `aisec-judge` | ✅ 17 + 3 passed | rule/regex/LLM consensus |
| `aisec-report` | ✅ 10 + 5 passed | HTML/JSON/SARIF/PDF |
| `aisec-integration-tests` (`mvp_flow`) | ✅ 1 passed | full 7-step flow |

---

## 5. Out-of-scope failures (NOT on the MVP flow path; not fixed here)

Per the task ("only fix blockers preventing the MVP flow"), these were observed but left as-is:

| Item | Status | Why out of scope |
|------|--------|------------------|
| `aisec-auth` standalone test compile (`tokio process` feature) | Broken on this branch | Auth is not used by the unauthenticated MVP flow; fixed separately in the auth PR (#7), not merged into this validation branch. |
| `aisec-plugin-host` 1 failing unit test (`path_glob`) | 1/6 fails | Plugins are not part of the MVP scan flow. |
| `aisec-models` `--features llama` build needs `CC=gcc CXX=g++` + libstdc++ link path | Env-specific | Local-model inference is optional; the MVP judge runs deterministically (rule+regex), so no model is required for the flow. |

---

## 6. Conclusion

The complete AISec MVP scan flow — **create project → add target → discover → detect AI endpoints →
prompt-injection attack → evaluate → report** — executes successfully end-to-end with real HTTP,
real persistence, and a real HTML report. Two blockers were fixed during validation (integration
crate wiring/`tracing`, and the `aisec-storage` test import); both are bug fixes, not new features.
All MVP-path crates' test suites are green, and the end-to-end harness (`mvp_flow.rs`) passes,
providing an ongoing regression guard for the flow.
