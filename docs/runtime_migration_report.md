# AISec Runtime Migration Report — Ollama → llama.cpp

**Date:** 2026-06-13  
**Scope:** Remove Ollama as the primary embedded runtime; migrate to `llama-server` + GGUF.

---

## Executive Summary

AISec previously embedded **Ollama** (`ollama serve` on port **11434**) as the local inference supervisor. The desktop app now uses **llama.cpp** (`llama-server` on port **8081**) with **GGUF vault models**. The `RuntimeSupervisor` public API is unchanged; only its implementation was replaced.

**Judge Engine (`aisec-judge`)** — no API changes.  
**ModelProvider (`aisec-runtime::EmbeddedModelProvider`)** — no trait changes.  
**Frontend** — no workflow changes (IPC DTO field names preserved for compatibility).

---

## Ollama Touchpoints Audited

### Removed / Replaced (embedded path)

| File | Lines (approx) | Previous behavior | Migration |
|------|----------------|-------------------|-----------|
| `src-tauri/src/ollama_runtime.rs` | — | Resolved `runtime/ollama`, started Ollama | **Deleted** → `embedded_runtime.rs` |
| `crates/aisec-runtime/src/supervisor.rs` | 113–128 | `ollama serve`, `OLLAMA_MODELS` | Spawns `LlamaCppRuntime` (`llama-server -m …`) |
| `crates/aisec-runtime/src/config.rs` | 12, 35–47 | Default `http://127.0.0.1:11434` | Default `http://127.0.0.1:8081` via `AISEC_LLAMA_*` |
| `crates/aisec-runtime/src/paths.rs` | 11–17 | `bundled_ollama_binary()` | `bundled_llama_server_binary()` (Ollama fn deprecated) |
| `crates/aisec-runtime/src/discovery.rs` | 33–85 | `GET /api/tags` (Ollama) | Recursive vault scan for `.gguf` files |
| `runtime/README.md` | — | Ollama binary instructions | llama-server + GGUF quant notes |

### Retained (legacy / non-embedded)

| File | Reason kept |
|------|-------------|
| `crates/aisec-models/src/runtime/ollama.rs` | Legacy vault entries with `ModelSource::Ollama` still route through HTTP client |
| `crates/aisec-judge/src/config.rs` | `LocalProvider::Ollama` enum value unchanged for persisted judge configs |
| `src/shared/ipc/judge.ts` | Frontend defaults unchanged |
| `resources/models.json` | `ollama-llama3` catalog entry unchanged; install now returns deprecation error |
| `crates/aisec-fingerprint/.../ollama.rs` | Target fingerprinting for external Ollama deployments (pentest scope) |

### localhost:11434 References

| Location | Status |
|----------|--------|
| `aisec-models/runtime/ollama.rs` | Legacy inference client only |
| `aisec-judge/config.rs` | Default in persisted config schema |
| `src/shared/ipc/judge.ts` | UI default (unchanged) |
| `aisec-fingerprint` | External target detection |

No embedded supervisor code paths use port **11434** after migration.

---

## New Components

| Path | Role |
|------|------|
| `crates/aisec-runtime/src/runtime/llama_cpp_runtime.rs` | `load_model`, `unload_model`, `infer`, `health`, `shutdown` |
| `crates/aisec-runtime/src/runtime/gguf.rs` | Q4/Q5/Q6/Q8 detection from filename |
| `src-tauri/src/embedded_runtime.rs` | Binary resolution: bundle → dev → `PATH` |

---

## RuntimeSupervisor API (stable)

| Method | Behavior (v2) |
|--------|----------------|
| `ensure_running()` | Verify `llama-server` binary; idle-ready if vault has GGUF |
| `ensure_model_loaded(path)` | **New** — spawn server for GGUF |
| `check_health()` | `GET /health` when loaded; else vault has GGUF |
| `list_installed_models()` | Scan `{data}/models/**/*.gguf` |
| `stop()` / `restart()` | Shutdown `llama-server` subprocess |
| `base_url()` | `http://127.0.0.1:8081` (configurable) |

---

## Judge Integration (unchanged API)

```
JudgeEngine
  → ModelProviderRuntime
    → EmbeddedModelProvider
      → LocalModelManager::inference_engine()
        → LlamaCppRuntime (aisec-models) for GGUF vault entries
```

Tauri `judge_config.rs` now calls `ensure_model_loaded()` for `LocalProvider::LlamaCpp` before building the judge. Legacy `LocalProvider::Ollama` still calls `ensure_running()` only.

---

## Catalog / Install Changes (backend only)

- `models_install` for `provider: "ollama"` → returns error directing users to HuggingFace GGUF or import.
- `models_install` IPC still accepts `ollamaBaseUrl` (ignored for deprecated path).
- `LocalModelManager::with_llama_binary()` wires bundled binary into vault inference.

---

## Binary Layout

```
runtime/
  llama-server       # macOS / Linux
  llama-server.exe   # Windows
```

Bundled via `src-tauri/tauri.conf.json` → `{resource_dir}/runtime/`.

---

## Environment Variables

| Old | New |
|-----|-----|
| `OLLAMA_HOST` | `AISEC_LLAMA_BASE_URL` / `AISEC_LLAMA_HOST` / `AISEC_LLAMA_PORT` |
| `OLLAMA_MODELS` | `{app_data}/models/` (unchanged path, GGUF vault) |

---

## Verification

```bash
cargo build --workspace
cargo test -p aisec-runtime
```

Place `llama-server` under `runtime/` and a Q4_K_M GGUF in the vault to exercise full inference.

---

## Follow-up (out of scope)

- Remove `aisec-models::OllamaRuntime` after vault migration tool
- Update `docs/RUNTIME.md` and frontend labels from "Ollama" to "llama.cpp"
- Native file picker for GGUF import
- Deduplicate `aisec-runtime` vs `aisec-models` `LlamaCppRuntime` into single shared instance
