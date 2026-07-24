# PromptLab Architecture & Implementation Audit Report

**Date:** 2026-06-13  
**Revision:** 2 (post Model Registry + Judge Runtime + Embedded Runtime integration)  
**Scope:** Source-code verification only (read-only audit).  
**Method:** Treat every feature as unimplemented unless proven in code. Evidence cites file path and line numbers.

---

## Executive Summary

Since the first audit (score **57/100**), the desktop app has closed several architecture gaps. **`resources/models.json` is loaded at startup** and drives browse/install/download. **`RuntimeSupervisor` is wired into `AppState`** with IPC (`runtime_status`, `runtime_restart`, `runtime_stop`) and judge Ollama lifecycle. **Judge local modes route through `ModelProvider` → `EmbeddedModelProvider` → `LocalModelManager`**, not direct Ollama/llama.cpp coupling. **Harness-normalized responses flow end-to-end** from `HarnessFactory` through `TransportResponse` to `judge_normalized`. **`promptlab-attack` no longer ships a parallel `HttpTransport` path** — scanner and default executor use `HarnessTransport`.

Remaining gaps: **no bundled Ollama binary** (`runtime/ollama` absent), **`HarnessRegistry` unused in production**, **model import UI uses manual file paths** (no native dialog), **registry entry URL quality** (Qwen GGUF 404 observed at runtime), **`promptlab-plugin-host` not wired to Tauri**, and **legacy plaintext descriptors** may still exist for records created before credential sanitization.

**Overall score: 72/100** (was 57/100)

---

## Changes Since Revision 1

| Area | Rev 1 | Rev 2 |
|------|-------|-------|
| `resources/models.json` loaded | ❌ | ✅ `model_registry.rs:35-48`, `lib.rs` startup |
| `promptlab-runtime` in Tauri | ❌ | ✅ `state.rs:23-46`, `runtime.rs`, `judge_config.rs:93-98` |
| Judge → ModelProvider bridge | ❌ | ✅ `factory.rs:99-129`, `judge_config.rs:76-105` |
| Normalized → Judge path | ⚠️ rebuilt from raw body | ✅ `attack.rs:126-134` |
| `promptlab-attack` direct reqwest | ❌ parallel path | ✅ removed; `HarnessTransport` only |
| GGUF/ZIP import IPC | ❌ | ✅ `models.rs:271+`, `ModelsPage.tsx` |
| Download pause/resume/cancel | ⚠️ backend only | ✅ IPC + UI polling |
| `promptlab-browser` crate | ⚠️ unwired duplicate | ✅ removed; consolidated into `promptlab-auth` |
| Credential encryption | ❌ plaintext | ⚠️ OS keychain + AES vault on new saves |
| Bundled Ollama | ❌ | ❌ still missing |

---

## SECTION 1 — Harness Architecture

### Crate existence: `crates/promptlab-harness`

| Component | Status | Evidence |
|-----------|--------|----------|
| `Harness` trait | ✅ IMPLEMENTED | `crates/promptlab-harness/src/traits/harness.rs` |
| `HarnessFactory` | ✅ IMPLEMENTED | `crates/promptlab-harness/src/factory/harness_factory.rs` |
| `HarnessRegistry` | ✅ IMPLEMENTED (library only) | `crates/promptlab-harness/src/registry/harness_registry.rs` |
| `HttpHarness` | ✅ IMPLEMENTED | `crates/promptlab-harness/src/providers/http.rs` |
| `PlaywrightHarness` | ✅ IMPLEMENTED | `crates/promptlab-harness/src/providers/playwright.rs` |
| `OpenAiHarness` | ✅ IMPLEMENTED | `crates/promptlab-harness/src/providers/openai.rs` |

### Q1: Does Attack Engine execute through Harness, or still call reqwest directly?

**Answer: ✅ Through Harness on all production paths.**

**Desktop scan/attack:**

- `src-tauri/src/harness_runtime.rs:37-76` — `build_harness_attack_runtime_parts` → `HarnessFactory`
- `src-tauri/src/commands/attack.rs:108-136` — executor + `judge_normalized`

**`promptlab-attack` library:**

- `crates/promptlab-attack/src/lib.rs:42-47` — `default_executor_for` uses `HarnessTransport`
- `crates/promptlab-attack/src/scanner.rs:8-9,88-89` — scanner uses `HarnessTransport` → `HarnessFactory`
- `crates/promptlab-attack/src/transport/harness.rs:82-101` — forwards `NormalizedResponse` from harness
- **No `HttpTransport` module** remains in `promptlab-attack`

**Note:** `HttpHarness` still uses `reqwest` internally — that is harness-layer HTTP, not bypassing the abstraction.

### Q2: Does `HarnessFactory` resolve providers?

**Status: ✅ IMPLEMENTED** — `harness_factory.rs:37-61`; Playwright injection in `harness_runtime.rs:78-86`.

### Q3: Are providers real or placeholders?

**Status: ✅ IMPLEMENTED** — live HTTP, OpenAI wrapper, Playwright driver integration.

### Q4: Any TODO in harness?

**Status: ✅ No TODO/FIXME in production `promptlab-harness` sources.**

### Q5: Mock harnesses / registry usage?

**Status: ⚠️ PARTIALLY IMPLEMENTED**

- `MockTransport` for tests only (`crates/promptlab-attack/src/transport/mock.rs`)
- **`HarnessRegistry` production usage: ❌ NOT IMPLEMENTED** — Tauri uses `HarnessFactory` only

---

## SECTION 2 — Normalized Response

### `NormalizedResponse` existence

**Status: ✅ IMPLEMENTED** — `crates/promptlab-harness/src/models/normalized_response.rs`

### Q1: Do all harnesses return normalized responses?

**Status: ✅ IMPLEMENTED** — all providers + factory normalizer.

### Q2: Does Judge consume normalized responses only?

**Status: ✅ IMPLEMENTED**

- `crates/promptlab-judge/src/engine.rs` — `judge_normalized` → `JudgeRequest::from_normalized`
- `crates/promptlab-attack/src/transport/mod.rs:19-26` — `TransportResponse` carries `normalized`
- `src-tauri/src/commands/attack.rs:126-134` — `let normalized = &attempt.response.normalized; judge.judge_normalized(..., normalized)`

**Rev 1 gap closed:** judge no longer rebuilds from raw HTTP body via `NormalizedResponse::from_http` on the attack IPC path.

### Q3: Does Judge still depend on raw HTTP responses?

**Status: ⚠️ PARTIALLY IMPLEMENTED**

- `JudgeRequest::from_normalized` embeds `raw_response` in context (`types.rs:199-203`) — intentional audit trail
- Attack results persist status + body snippets in `response_json` (`attack.rs:148+`)

---

## SECTION 3 — Auth Session Framework

### Crate: `promptlab-auth` (replaces standalone `promptlab-browser`)

**Status: ✅ IMPLEMENTED and wired**

- `promptlab-browser` **removed from workspace** — `Cargo.toml:3-18` (no member)
- Session APIs live in `promptlab-auth`; Tauri uses `auth-vault` + `AuthSessionManager`
- `src-tauri/src/harness_runtime.rs:44-46` — `resolve_descriptor_for_runtime` before harness build

### Q1: Are browser sessions persisted?

**Status: ✅ IMPLEMENTED**

- Finish: `src-tauri/src/commands/auth.rs`
- Encrypted disk: `crates/promptlab-auth/src/secrets/vault.rs:54-55` — `{session_id}.storage.enc`
- DB: `auth_sessions` table + migration `006_auth_secure_credentials.sql`

### Q2: Where stored?

- **Encrypted vault:** `{data_dir}/auth-vault/{session_id}.storage.enc`
- **Master key:** OS keychain via `SecretStore` (`vault.rs:25-38`)
- **Database:** `auth_sessions` with `credential_reference_id`

### Q3–Q5: Cookies / localStorage / sessionStorage

**Status: ✅ IMPLEMENTED (Playwright storageState + token scraping)**

- Runner: `crates/promptlab-auth/playwright/runner.mjs`
- Types: `crates/promptlab-auth/src/types.rs`

### Q6: Plaintext passwords?

**Status: ⚠️ PARTIALLY IMPLEMENTED**

- **New targets:** `sanitize_target_descriptor` strips secrets to OS keychain on `target_create` — `domain.rs:60-61`, `descriptor.rs:11-18`
- **UI still collects plaintext** in wizard form before IPC — `targetDescriptor.ts:214,239` (transient; not stored inline after sanitization)
- **Legacy rows** may still contain inline secrets from pre-migration records
- **Judge config** still plaintext JSON — `judge_config.rs:41-45`

---

## SECTION 4 — Scan Wizard Auth Flow (Step 2)

| Question | Status | Evidence |
|----------|--------|----------|
| Record session UI | ✅ | `TargetFormFields.tsx`, `PlaywrightRecordPanel.tsx` |
| Launch browser | ✅ | `auth_record_session_start` |
| Sessions saved | ✅ | `auth_record_session_finish` |
| Wizard state persists on nav | ✅ | `wizardState.ts` (browser `sessionStorage`) |

---

## SECTION 5 — Discovery Integration

| Question | Status | Evidence |
|----------|--------|----------|
| Authenticated sessions | ✅ | `discovery.rs`, `session_auth.rs` |
| Inject browser session state | ✅ | harness auth headers + storage state path |
| Anonymous when no session | ✅ | `harness_runtime.rs:51-73` |

---

## SECTION 6 — Attack Engine Integration

### Call graph (desktop production)

```
scan.rs / attack.rs IPC
  → build_harness_attack_runtime (harness_runtime.rs)
    → HarnessFactory (+ Playwright when needed)
    → HarnessTransport (promptlab-attack)
  → AttackExecutor::execute_category
    → HarnessFactory::execute → NormalizedResponse
  → judge.judge_normalized(normalized)
    → build_configured_judge_engine (judge_config.rs)
      → ModelProviderRuntime (local modes)
```

| Question | Status |
|----------|--------|
| reqwest directly (attack library) | ✅ removed — `HarnessTransport` only |
| Through HarnessFactory (desktop) | ✅ |
| Browser targets | ✅ Playwright harness when session present |
| API targets | ✅ Http/OpenAi harnesses |

---

## SECTION 7 — Local AI Runtime

Crate: `crates/promptlab-runtime` — ✅ library + **Tauri integration**

| Question | Status | Evidence |
|----------|--------|----------|
| `RuntimeSupervisor` in app | ✅ | `state.rs:23-46` |
| IPC status/restart/stop | ✅ | `commands/runtime.rs`, `lib.rs:187-189` |
| Judge Ollama lifecycle | ✅ | `judge_config.rs:93-98` — `ensure_running()` |
| Ollama bundled | ❌ | `runtime/` has `.gitkeep` only; log: "embedded Ollama runtime not found" |
| System Ollama fallback | ⚠️ | `ollama_runtime.rs:52-57` — `which ollama` when bundled missing |
| Health in UI | ⚠️ | IPC exists; surfacing depends on frontend usage |

---

## SECTION 8 — Model Management

| Question | Status | Evidence |
|----------|--------|----------|
| Models page | ✅ | `ModelsPage.tsx` |
| Install / remove | ✅ | `models_install`, `models_remove` |
| Activate for judge | ✅ | `localVaultModelId` + `resolve_judge_local_settings` |
| GGUF import IPC | ✅ | `models_import_gguf` — `models.rs:271+` |
| ZIP import IPC | ✅ | `models_import_zip` |
| Import UI | ⚠️ | Manual path fields — `ModelsPage.tsx:346+` (no native file dialog) |
| Resumable downloads | ✅ | `DownloadCoordinator`; IPC pause/resume/cancel |
| Download UI | ⚠️ | Progress polling; no speed/ETA |
| Browse catalog source | ✅ | `LocalModelManager::browse_catalog()` from loaded registry — `manager.rs:118-120` |

**Runtime issue observed:** HuggingFace 404 for `Qwen/Qwen3-8B-GGUF` entry in `resources/models.json` during dev session — registry metadata may need URL/path correction.

---

## SECTION 9 — Built-in Model Registry

| Question | Status | Evidence |
|----------|--------|----------|
| `resources/models.json` exists | ✅ | `resources/models.json:1-29` |
| Loaded by running app | ✅ | `model_registry.rs:35-48`; bundled in `tauri.conf.json` |
| Optional remote merge | ✅ | `PROMPTLAB_MODEL_REGISTRY_URL` — `model_registry.rs:29-33` |
| `models_registry_info` IPC | ✅ | `models.rs:223+`, `ModelsPage.tsx:315-322` |
| Hardcoded `curated_catalog()` | ⚠️ deprecated | `catalog.rs:14-17` — returns empty; not used when manager has catalog |
| Offline browse | ✅ | Loaded catalog slice |
| `BuiltinModelRegistry` in `promptlab-runtime` | ⚠️ | Separate crate registry; desktop uses `promptlab-models::BuiltinCatalog` |

---

## SECTION 10 — Judge Engine

| Question | Status | Evidence |
|----------|--------|----------|
| Modes | ✅ | Deterministic, LocalLlm, RemoteLlm, Consensus |
| Default | ✅ Deterministic | `config.rs` |
| Local via runtime bridge | ✅ | `JudgeRuntimeContext` + `ModelProviderRuntime` — `factory.rs:99-129` |
| Direct Ollama/LlamaCpp in judge | ✅ removed | No `OllamaRuntime`/`LlamaCppRuntime` in factory |
| Structured JSON output | ✅ | `JudgeStructuredOutput`, `to_json_string()` — `types.rs:252-293` |
| Normalized input | ✅ | `attack.rs:126-134` |

Production path: `build_configured_judge_engine` → `prepare_judge_runtime_context` → `build_judge_engine`.

---

## SECTION 11 — Database Integration

| Entity | SQLite | Loaded via IPC |
|--------|--------|----------------|
| Projects | ✅ | ✅ |
| Targets | ✅ | ✅ (descriptor sanitized on create) |
| Scans | ✅ | ✅ |
| Endpoints | ✅ | ✅ |
| Findings | ✅ | ✅ |
| Reports | ✅ | ✅ |
| Auth sessions | ✅ + credential refs | ✅ |
| Models table | ⚠️ schema exists | ❌ app uses file vault |

Wizard state: browser `sessionStorage` (not SQLite).

---

## SECTION 12 — Mock Detection

| Location | Type |
|----------|------|
| `TopBar.tsx` | UI "Mock mode" when IPC unavailable |
| `promptlab-attack/transport/mock.rs` | Test mock |
| `promptlab-models/runtime/mock.rs` | Test mock |
| `promptlab-judge/mock_runtime.rs` | Test mock |
| `curated_catalog()` | Deprecated empty stub — not production catalog source |

No `todo!()` / `unimplemented!()` in production hot paths.

---

## SECTION 13 — Security Review

| Check | Status | Evidence |
|-------|--------|----------|
| Credentials in new target descriptors | ⚠️ sanitized on save | `domain.rs:60-61`, `descriptor.rs:11-18` |
| UI plaintext entry (transient) | ⚠️ | `targetDescriptor.ts:214,239` |
| Legacy inline secrets | ⚠️ may exist | pre-migration rows |
| Session encryption at rest | ✅ | `EncryptedVault` AES-256-GCM — `vault.rs:17-56` |
| Master key in OS keychain | ✅ | `SecretStore` + `keyring` |
| Auth migration | ✅ | `secrets/migrate.rs`; log "migrated legacy auth secrets" |
| Judge config plaintext | ⚠️ | `judge_config.rs:41-45` |
| Secrets logged | ⚠️ NOT VERIFIED | debug logs use ids/paths only in store paths |

---

## SECTION 14 — Architecture Compliance Score

| Area | Rev 1 | Rev 2 | Rationale |
|------|-------|-------|-----------|
| Harness Architecture | 68 | **78** | All attack paths via harness; normalized preserved; registry still unused |
| Auth Session Framework | 52 | **68** | Encrypted vault + keychain; browser crate consolidated; legacy/plain UI gaps |
| Runtime | 20 | **55** | Supervisor wired + IPC + judge; no bundled binary |
| Model Management | 55 | **78** | Registry loaded; import/download IPC; manual import UI; bad HF URL |
| Judge Engine | 70 | **82** | ModelProvider bridge; structured JSON; normalized path fixed |
| UI Integration | 63 | **72** | Registry info, runtime commands, download controls |
| Database Integration | 72 | **74** | Credential ref migration; wizard still sessionStorage |
| **Overall** | **57** | **72** | |

---

## SECTION 15 — Gap Report (Top 20)

### Critical

1. **No bundled Ollama binary** — `runtime/ollama` missing; local Ollama install path blocked without system binary — `ollama_runtime.rs:60+`
2. **Registry URL quality** — Qwen GGUF download 404 at runtime; fix `resources/models.json` or resolver — observed in dev logs
3. **Legacy plaintext descriptors** — records created before `sanitize_target_descriptor` may retain inline secrets

### High

4. **`HarnessRegistry` unused at runtime** — production uses factory only
5. **Model import without native file picker** — manual path input — `ModelsPage.tsx`
6. **`promptlab-plugin-host` not wired to Tauri** — no references under `src-tauri/`
7. **Platform-specific harnesses** (Dify, OpenWebUI, MCP) — not dedicated providers
8. **SQLite `models` table unused** — vault is filesystem-based

### Medium

9. Download UI lacks speed/ETA — progress bytes only
10. Judge config stored as plaintext JSON — `judge_config.rs`
11. Wizard state in `sessionStorage` only — not durable across browser profiles
12. `BuiltinModelRegistry` in `promptlab-runtime` vs `BuiltinCatalog` in `promptlab-models` — dual registry types
13. System Ollama fallback may surprise users expecting embedded-only behavior
14. Attack results still embed raw body snippets in `response_json`
15. Documentation drift — `docs/ARCHITECTURE.md` still references `promptlab-browser` crate

### Low

16. Deprecated `curated_catalog()` stub remains in tree
17. Compiler warnings in several crates (unused imports)
18. UI label "Mock mode" when backend offline
19. No dedicated runtime health widget verified in all pages
20. Integration test suite not fully green workspace-wide (pre-existing crate test gaps per AGENTS.md)

---

## Compliance Matrix (Summary)

| Requirement | Rev 1 | Rev 2 |
|-------------|-------|-------|
| `promptlab-harness` + trait/factory/providers | ✅ | ✅ |
| Desktop attacks via harness | ✅ | ✅ |
| Library attacks always via harness | ❌ | ✅ |
| All harnesses return `NormalizedResponse` | ✅ | ✅ |
| Judge consumes harness output end-to-end | ⚠️ | ✅ |
| Auth sessions persisted + encrypted | ⚠️ | ✅ (new path) |
| Wizard Step 2 auth recording | ✅ | ✅ |
| Discovery authenticated sessions | ✅ | ✅ |
| `promptlab-runtime` operational in app | ❌ | ✅ (supervisor; binary missing) |
| `resources/models.json` drives catalog | ❌ | ✅ |
| SQLite persistence for core entities | ✅ | ✅ |
| Credential encryption | ❌ | ⚠️ (new saves + vault; legacy rows) |
| Judge via ModelProvider / runtime bridge | ❌ | ✅ |
| Model import + controlled download IPC | ❌ | ✅ |

---

## Audit Conclusion

The codebase now **substantially matches** the intended PromptLab architecture for the desktop hot path: harness-based attacks with preserved normalization, encrypted auth sessions, embedded runtime supervision wired through Tauri IPC, registry-driven model management, and judge inference through the `ModelProvider` bridge.

Primary remaining work: **ship the Ollama binary**, **fix registry entry URLs**, **native import UX**, **plugin host wiring**, and **migrate/audit legacy plaintext credential rows**.

**Overall score: 72/100** (up from 57/100 on 2026-06-13 revision 1)
