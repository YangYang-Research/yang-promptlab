# AI Runtime Single Source of Truth — Migration Report

**Date:** 2026-06-13  
**Scope:** Architecture refactor — unified AI configuration via AI Runtime Service

---

## Summary

AISec now has **one** persisted AI configuration: `ai_inference_settings.json` (`AiInferenceSettings`). All inference-consuming features route through **`AiRuntimeService`** (`src-tauri/src/ai_runtime_service.rs`).

Legacy `judge_config.json` is migrated automatically on startup and renamed to `judge_config.json.migrated`.

---

## New Architecture

```
Judge / Planner / Generator / Agent scan
              │
              ▼
      AiRuntimeService
   (complete, health, build_judge_engine, build_llm_backend)
              │
              ▼
   ai_inference_settings.json  +  model registry
              │
      ┌───────┴───────┐
      ▼               ▼
 Third-party      Local llama-server
 (RemoteLlm)      (LocalLlm via ModelProvider)
```

**Forbidden in feature modules:** direct provider clients, reading `judge_config.json`, per-feature model/provider selection.

---

## Files Changed

### Added

| File | Purpose |
|------|---------|
| `src-tauri/src/ai_runtime_service.rs` | Single AI Runtime service (SSOT) |
| `docs/AI_RUNTIME_SSOT_MIGRATION_REPORT.md` | This report |

### Removed

| File | Reason |
|------|--------|
| `src-tauri/src/commands/judge.rs` | Judge configuration IPC removed |
| `src/features/judge/JudgeProviderPage.tsx` | UI consolidated into AI Runtime |
| `src/shared/ipc/judge.ts` | Judge IPC client removed |

### Modified (backend)

| File | Change |
|------|--------|
| `src-tauri/src/lib.rs` | Register `ai_runtime_service`; startup migration; remove judge commands; add `runtime_test_*` |
| `src-tauri/src/judge_config.rs` | Legacy I/O + secrets migration only (no engine build) |
| `src-tauri/src/commands/attack.rs` | `build_judge_engine_from_runtime` |
| `src-tauri/src/commands/planner.rs` | `build_planner_llm` via runtime service |
| `src-tauri/src/commands/generator.rs` | `build_generator_llm` via runtime service |
| `src-tauri/src/agent_service.rs` | Agent plan/generate use runtime when LLM modes + runtime ready |
| `src-tauri/src/commands/runtime.rs` | `runtime_test_connectivity`, `runtime_test_inference` |
| `src-tauri/src/commands/security.rs` | Removed judge save sanitizer |
| `src-tauri/src/commands/mod.rs` | Dropped `judge` module |
| `src-tauri/src/commands/scan.rs` | `ScanAgentHost.planner_mode` from agent config |

### Modified (frontend)

| File | Change |
|------|--------|
| `src/app/router/nav.ts` | Removed Judge Provider nav item |
| `src/app/router/AppRouter.tsx` | Removed `/judge` route |
| `src/shared/ipc/index.ts` | Removed judge exports |
| `src/shared/ipc/runtime.ts` | `testRuntimeConnectivity`, `testRuntimeInference` |
| `src/features/settings/SettingsPage.tsx` | AI Runtime card + link; updated copy |

---

## Legacy Code Removed

- Tauri commands: `judge_config_get`, `judge_config_save`, `judge_test_connectivity`, `judge_test_model`
- `build_configured_judge_engine`, `prepare_judge_runtime_context`, `resolve_judge_local_settings` from active path
- `build_planner_llm_backend` / `build_generator_llm_backend` (duplicated judge-config wiring)
- Judge Provider page and navigation entry
- Frontend `judge.ts` IPC module

---

## Data Migration

### Automatic (on app startup)

1. **`migrate_judge_config_secrets`** — existing: move plaintext API keys from legacy judge file to keychain (unchanged).
2. **`migrate_legacy_judge_config`** — **new**:
   - Reads `judge_config.json` if present
   - Maps `RemoteLlm` → `AiInferenceRoute::ThirdParty` + matching registry model
   - Maps `LocalLlm`/`Consensus` → `AiInferenceRoute::Local` + `vault_model_id`
   - Writes `ai_inference_settings.json`
   - Renames legacy file to `judge_config.json.migrated`

### Single persistence

| Before | After |
|--------|-------|
| `judge_config.json` | **Removed** (migrated) |
| `ai_inference_settings.json` | **Only** AI config |

Model credentials for third-party APIs remain in **model registry metadata** + encrypted vault (unchanged).

---

## IPC Changes

### Removed

- `judge_config_get`, `judge_config_save`, `judge_test_connectivity`, `judge_test_model`

### Added

- `runtime_test_connectivity` — health check via AI Runtime
- `runtime_test_inference` — smoke inference via AI Runtime

### Unchanged (runtime SSOT)

- `runtime_configuration`, `runtime_inference_settings`, `runtime_set_inference_route`
- All runtime lifecycle commands (`runtime_install`, `runtime_start`, etc.)

---

## Validation Checklist

| Requirement | Status |
|-------------|--------|
| Exactly one AI Runtime configuration | ✅ `ai_inference_settings.json` |
| Judge uses AI Runtime | ✅ `attack.rs` → `build_judge_engine_from_runtime` |
| Planner uses AI Runtime (LLM mode) | ✅ `planner.rs` → `build_planner_llm` |
| Generator uses AI Runtime (LLM mode) | ✅ `generator.rs` → `build_generator_llm` |
| Agent scan uses AI Runtime | ✅ `agent_service.rs` |
| No judge configuration IPC | ✅ commands removed |
| No Judge Provider page | ✅ removed |
| Legacy judge config migrated | ✅ startup migration |
| `cargo check -p aisec-desktop` | ✅ passes |
| `npm run build` | ✅ passes |

---

## Remaining TODOs

| Item | Notes |
|------|-------|
| Fingerprint LLM usage | Fingerprint engine is rule-based HTTP; no separate LLM config existed — N/A |
| Report generator AI | Reports remain deterministic (`aisec-report`); no LLM path today |
| `stream()` on AiRuntimeService | Not exposed yet; all backends use `stream: false` |
| `embed()` on AiRuntimeService | Embeddings IPC exists (`models_test_embeddings`) but not wired through runtime service |
| AIRuntimePage test buttons | IPC added; UI can call `testRuntimeConnectivity` / `testRuntimeInference` (optional UX follow-up) |
| Security audit label | Still reports `judgeConfigLegacy` for unmigrated `judge_config.json` files |
| `autoJudge` setting | Client-only toggle in AppStore; does not select provider — kept with clarified label |

---

## Restart Note

After pulling this refactor, restart `npm run tauri dev` so the Rust backend reloads with new IPC surface and startup migration runs.
