# PromptLab Model Registry v2 (GGUF-first)

**Version:** 2.0  
**Date:** 2026-06-13  
**Scope:** `resources/models.json` — embedded llama.cpp runtime catalog

---

## Overview

The built-in model registry is **GGUF-first**. Every catalog entry targets the embedded **llama.cpp** engine. Ollama tags, `ollama pull`, and HuggingFace `repo`/`file` indirection are removed from the registry schema.

Downloads use a direct **`download_url`** (typically HuggingFace `resolve/main/...gguf`).

---

## Schema

```json
{
  "models": [
    {
      "id": "qwen3-8b-judge",
      "name": "Qwen3 8B Security Judge",
      "purpose": "judge",
      "recommended": true,
      "engine": "llama.cpp",
      "format": "gguf",
      "size": "4.7GB",
      "sha256": "",
      "download_url": "https://huggingface.co/Qwen/Qwen3-8B-GGUF/resolve/main/qwen3-8b-q4_k_m.gguf"
    }
  ]
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `id` | yes | Unique catalog identifier |
| `name` | yes | Display name |
| `purpose` | no | `judge`, `general`, etc. |
| `recommended` | no | Highlight in browse UI |
| `engine` | yes | Must be `llama.cpp` |
| `format` | yes | Must be `gguf` |
| `size` | no | Human label (`4.7GB`) |
| `sha256` | no | Expected checksum (64 hex); empty skips verify |
| `download_url` | yes | HTTPS URL to `.gguf` file |

---

## Startup Validation

On app startup (`BuiltinCatalog::load_with_optional_remote`):

1. Parse `resources/models.json`
2. Optionally merge remote registry (`PROMPTLAB_MODEL_REGISTRY_URL`)
3. Run `validate_registry()` — `crates/promptlab-models/src/registry_validate.rs`

### Checks

| Check | Failure message |
|-------|-----------------|
| Duplicate `id` | `duplicate id` |
| Empty `id` / `name` | field required |
| `engine` ≠ `llama.cpp` | unsupported engine |
| `format` ≠ `gguf` | unsupported format |
| Missing / non-HTTPS `download_url` | missing or invalid URL |
| Invalid `sha256` | not 64 hex chars |

Invalid entries are **excluded from browse** but reported in diagnostics.

---

## IPC

| Command | Returns |
|---------|---------|
| `models_registry_info` | Counts: `totalModels`, `validModels`, `invalidModels` |
| `models_registry_diagnostics` | Full report + issue list |
| `models_browse` | Valid catalog entries only |
| `models_download_start` | Resumable download from `download_url` |

---

## UI — Models Page

- **Registry Diagnostics** card: total / valid / invalid + per-field issues
- **Browse Registry**: GGUF entries with `engine`, `format`, `download_url`
- **Judge Provider**: vault models use `llama_cpp` only (no Ollama UI on Models page)

Scan Wizard is unchanged.

---

## Implementation Map

| Component | Path |
|-----------|------|
| Registry file | `resources/models.json` |
| Parser | `crates/promptlab-models/src/builtin_catalog.rs` |
| Validator | `crates/promptlab-models/src/registry_validate.rs` |
| Startup load | `src-tauri/src/model_registry.rs` |
| IPC | `src-tauri/src/commands/models.rs` |
| Frontend | `src/features/models/ModelsPage.tsx` |
| IPC client | `src/shared/ipc/models.ts` |

---

## Remote Merge

Set `PROMPTLAB_MODEL_REGISTRY_URL` to an HTTPS JSON file using the same v2 schema. Remote entries with new `id` values are appended; duplicates are skipped.

---

## Migration from v1

| v1 | v2 |
|----|-----|
| `provider: "huggingface"` + `repo` + `file` | `download_url` (full HTTPS) |
| `provider: "ollama"` + `ollamaTag` | **removed** — use GGUF entry |
| `ollama pull` install path | **removed** |
| Default port 11434 | N/A — llama.cpp vault only |

Legacy vault entries with `ModelSource::Ollama` may still exist on disk; new installs are GGUF-only.

---

## Bundled Catalog (current)

| id | Purpose | Quant |
|----|---------|-------|
| `qwen3-8b-judge` | judge (recommended) | Q4_K_M |
| `llama3-8b-q4` | general | Q4_K_M |
| `mistral-7b-q4` | general | Q4_K_M |

---

## Related Docs

- [runtime_architecture_v2.md](./runtime_architecture_v2.md)
- [runtime_migration_report.md](./runtime_migration_report.md)
- [MODEL_REGISTRY.md](./MODEL_REGISTRY.md) — prior IPC reference (partially superseded)
