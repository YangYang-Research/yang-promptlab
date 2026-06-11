# Code Hygiene Audit Report

**Date:** 2026-06-10  
**Scope:** Full repository (`crates/`, `src/`, `src-tauri/`, `tests/`, `packages/`, `plugins/`, `scripts/`, `examples/`)  
**Patterns searched:** `TODO`, `FIXME`, `panic!`, `unwrap()`, `expect()`, `unimplemented!()`, `todo!()`

---

## Executive Summary

| Pattern | Count (source code) | Risk |
|---------|---------------------|------|
| `TODO` / `FIXME` | **1** | Low |
| `panic!` | **1** | Test-only |
| `unimplemented!()` / `todo!()` | **0** | None |
| `.unwrap()` / `.expect()` | **~280** (Rust) | Mostly tests; **~30 in production paths** |
| Frontend (TS/TSX) | **0** | Clean |

The codebase has **no `todo!()` or `unimplemented!()` markers** and almost no TODO comments. The main hygiene concern is **production use of `unwrap()`/`expect()`** on fallible operations (especially JSON serialization in `aisec-auth`), and heavy test reliance on panicking assertions (standard Rust test style).

---

## 1. TODO / FIXME

### Source code (excluding docs)

| File | Line | Text |
|------|------|------|
| `crates/aisec-discovery/examples/verify_target.rs` | 15 | `// TODO: use 4+ after crawler deadlock fix` |

### Documentation references only

- `docs/DISCOVERY_VERIFICATION_REPORT.md` — mentions absence of TODO markers (meta, not a task)

**Assessment:** Effectively **zero outstanding TODO/FIXME debt** in production code. One TODO tracks a known crawler fix.

---

## 2. `unimplemented!()` / `todo!()`

**Result:** No matches in the repository.

No stub functions left as explicit compile-time or runtime placeholders via these macros.

---

## 3. `panic!`

| File | Line | Context | Classification |
|------|------|---------|----------------|
| `crates/aisec-attack/tests/integration.rs` | 59 | `.unwrap_or_else(\|e\| panic!("{category} failed: {e}"))` | **Test** — intentional fail-fast in category loop |

**Production code:** **0** explicit `panic!` macros.

**Implicit panics:** Production `unwrap()`/`expect()` calls (see §5) can still panic at runtime.

---

## 4. Frontend & Tauri Shell

| Area | TODO | FIXME | panic | unwrap | expect | unimplemented | todo |
|------|------|-------|-------|--------|--------|---------------|------|
| `src/` (React) | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `src-tauri/` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `packages/plugin-sdk-*` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `plugins/` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |

Frontend and Tauri bootstrap are clean under these patterns.

---

## 5. `.unwrap()` / `.expect()` — Overview

### Totals (Rust)

| Category | Approx. count | Files |
|----------|---------------|-------|
| **All Rust matches** | ~280 | 66 |
| **Dedicated test files** (`tests/`, `**/tests/*.rs`) | ~45 | 12 |
| **`#[cfg(test)]` modules inside `src/`** | ~200 | 40+ |
| **Examples** | 3 | 1 |
| **Production library code** | **~30** | 14 |

---

## 6. Production `.unwrap()` / `.expect()` — Detail

These occur in non-test library paths and **can panic** in production if invariants break.

### High priority — user/data path

| File | Lines | Usage | Risk |
|------|-------|-------|------|
| `crates/aisec-auth/src/session/store.rs` | 72–73, 101, 120, 182, 228, 237 | `serde_json::to_value(...).unwrap()`, `from_str(...).unwrap()` | **High** — corrupt DB JSON or non-serializable types crash auth store |
| `crates/aisec-auth/src/engine.rs` | 134 | `serde_json::to_value(storage_state).unwrap()` | **High** — replay session panics on serialization failure |
| `crates/aisec-auth/src/playwright/client.rs` | 233, 256 | `serde_json::to_value(req).unwrap()` on IPC payloads | **Medium** — should map to `AisecResult` |
| `crates/aisec-models/src/registry.rs` | 105 | `self.entries.get(id).expect("entry exists")` | **Medium** — internal API; caller may expect `Err` not panic |

### Medium priority — infrastructure / defaults

| File | Lines | Usage | Risk |
|------|-------|-------|------|
| `crates/aisec-attack/src/collector.rs` | 38, 42, 48, 66, 75, 77 | `Mutex::lock().unwrap()` | **Medium** — poisoned mutex panics; use in production collector |
| `crates/aisec-attack/src/transport/mock.rs` | 33, 40, 41 | `Mutex::lock().unwrap()` | **Low** — mock transport only |
| `crates/aisec-attack/src/transport/http.rs` | 20 | `Client::builder()...expect("reqwest client")` | **Low** — init-time; acceptable if documented |
| `crates/aisec-models/src/download/huggingface.rs` | 19 | `expect("reqwest client")` | **Low** — init-time |
| `crates/aisec-payload/src/pipeline.rs` | 163 | `Default` impl: `with_defaults().expect(...)` | **Medium** — `PayloadPipeline::default()` panics if catalog broken |
| `crates/aisec-payload/src/library/mod.rs` | 125 | `builtin().expect("embedded catalog must be valid")` | **Low** — static embed; compile-time invariant |
| `crates/aisec-report/src/engine.rs` | 81 | `Default`: `new("./data/reports").expect(...)` | **Medium** — panics if reports dir not creatable |

### Low priority — static / algorithmic invariants

| File | Lines | Usage | Risk |
|------|-------|-------|------|
| `crates/aisec-discovery/src/detectors/ai.rs` | 16 | `Regex::new(...).expect("regex")` | **Low** — static pattern |
| `crates/aisec-discovery/src/detectors/api.rs` | 8 | `Regex::new(...).expect("regex")` | **Low** — static pattern |
| `crates/aisec-fingerprint/src/evaluator.rs` | 95 | `Regex::new(pattern).expect(...)` in `OnceLock` | **Low** — bad rule pattern crashes at first use |
| `crates/aisec-fingerprint/src/engine.rs` | 122 | `partial_cmp(...).unwrap()` in sort | **Low** — NaN confidence would panic (unlikely) |

---

## 7. Test & Example `.unwrap()` / `.expect()` — By Crate

Standard Rust test pattern; acceptable in tests but contributes to audit count.

| Crate / area | Approx. matches | Notes |
|--------------|-----------------|-------|
| `aisec-plugin-host` | 28 | Sample plugin tests + manager tests |
| `aisec-storage` | 35+ | Repository CRUD tests; lib integration test |
| `aisec-report` | 25+ | Formatter + engine integration |
| `aisec-models` | 30+ | Download, verify, manager, integration |
| `aisec-attack` | 25+ | Executor, orchestrator, mutator, collector tests |
| `aisec-discovery` | 12+ | url_policy, crawler, integration |
| `aisec-judge` | 12+ | Engine, evaluators, integration |
| `aisec-payload` | 15+ | Pipeline, library, integration |
| `aisec-fingerprint` | 12+ | Engine provider tests |
| `aisec-auth` | 10+ | Engine tests (lines 318+ are `#[cfg(test)]`) |
| `tests/integration/` | 13 | Smoke, storage, auth persistence |
| Examples | 3 | `verify_target.rs` |

---

## 8. Findings by Severity

### Critical
None — no `unimplemented!()`, no production `panic!`, no `todo!()` in hot paths.

### High
1. **`aisec-auth` session store** — 7× JSON `unwrap()` on read/write paths; should use `?` and `StorageResult` / `AisecError`.
2. **`aisec-auth` engine replay** — storage state serialization `unwrap()` at line 134.

### Medium
1. **`ResultCollector` mutex unwraps** — production in-memory collector can panic on poisoned lock.
2. **`Default` impls** on `PayloadPipeline` and `ReportingEngine` — panic if initialization fails.
3. **`ModelRegistry::get_entry`** — `expect` instead of `Option`/`Result` for missing ID.
4. **Playwright client** — JSON serialization unwrap on outbound IPC.

### Low
1. Static regex `expect` in detectors (compile-time valid patterns).
2. HTTP client builder `expect` at initialization.
3. Test-only and example unwrap volume (~250) — normal for Rust tests; no action required unless moving to `?` in async tests.

---

## 9. Cross-Reference with Known Issues

| Hygiene item | Related known bug |
|--------------|-------------------|
| TODO in `verify_target.rs` | Crawler deadlock (`worker_count > 1`) — see `DISCOVERY_VERIFICATION_REPORT.md` |
| Auth JSON unwraps | No vault encryption; corrupt session data crashes vs. error |
| No TODOs in plugin host | Manifest schema drift not tracked in code comments |

---

## 10. Recommendations

| Priority | Action |
|----------|--------|
| P0 | Replace JSON `unwrap()` in `aisec-auth/src/session/store.rs` with `map_err` → `AisecError` |
| P0 | Fix `AuthEngine::replay_session` line 134 — propagate serialization error |
| P1 | Replace `Mutex::lock().unwrap()` in `ResultCollector` with `lock().map_err(...)` or `parking_lot` |
| P1 | Change `ModelRegistry` internal get to return `Option` / `Result` |
| P2 | Remove `Default` impls that panic; use `try_default()` or explicit `new()` only |
| P2 | Resolve discovery TODO after crawler fix; restore `worker_count: 8` in example |
| P3 | Add `#![deny(clippy::unwrap_used)]` to production modules incrementally (exclude tests) |
| P3 | Add CI grep gate: fail on new `unwrap()` outside `#[cfg(test)]` |

---

## 11. Search Commands Used

```bash
# Repository-wide (representative)
rg 'TODO|FIXME' --glob '!docs/**'
rg 'panic!' --glob '*.rs'
rg 'unimplemented!\(|todo!\(' --glob '*.rs'
rg '\.unwrap\(\)|\.expect\(' --glob '*.rs'
```

---

## 12. Conclusion

AISec source code is **remarkably free of TODO/FIXME and stub macros** (`todo!`, `unimplemented!`). Technical debt manifests primarily as:

1. **~30 production `unwrap`/`expect` calls**, concentrated in **`aisec-auth`** (JSON persistence) and **`aisec-attack`** (mutex collector).
2. **~250 test/example panicking assertions** — idiomatic Rust tests, not a product risk.
3. **One tracked TODO** for discovery crawler fix.

**Verdict:** Code hygiene is **good for a prototype**; **`aisec-auth` serialization unwraps** should be fixed before MVP ships auth/session features.

---

*Regenerate after significant refactors or before release gating.*
