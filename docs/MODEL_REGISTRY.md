# Model Registry

AISec loads its built-in model catalog from `resources/models.json` at startup. That file is the **source of truth** for browse/install flows in the Models page and for judge model resolution.

## Registry file

Path: [`resources/models.json`](../resources/models.json)

Each entry describes a model the app can install or reference:

| Field | Description |
|-------|-------------|
| `id` | Stable catalog id (used by IPC install/download) |
| `name` | Display name |
| `purpose` | Role hint (`judge`, `general`, …) |
| `recommended` | Highlight in UI when true |
| `provider` | `huggingface`, `ollama`, or `gguf` |
| `repo` / `file` | HuggingFace repo + GGUF filename |
| `ollamaTag` | Ollama model tag for `ollama pull` |
| `sha256` | Optional expected checksum |

Bundled builds include this file via Tauri resources (`resources/models.json`).

## Startup

On backend init (`src-tauri/src/lib.rs`):

1. Resolve registry path (bundled resource → repo fallback in dev).
2. Load `resources/models.json`.
3. Optionally merge a remote registry when `AISEC_MODEL_REGISTRY_URL` is set and reachable.
4. Attach the catalog to `LocalModelManager` in `AppState`.

If the remote URL is unavailable, startup continues offline with the bundled registry only.

## IPC commands

| Command | Purpose |
|---------|---------|
| `models_registry_info` | Registry metadata (entry count, remote merge) |
| `models_browse` | List catalog entries from the loaded registry |
| `models_install` | Install Ollama tag or blocking HuggingFace download |
| `models_import_gguf` | Import a local `.gguf` file into the vault |
| `models_import_zip` | Extract `.gguf` from a ZIP package into the vault |
| `models_download_start` | Background HuggingFace download for a catalog entry |
| `models_download_status` | Poll progress; auto-finalizes completed downloads |
| `models_download_pause` | Pause active download |
| `models_download_resume` | Resume paused download |
| `models_download_cancel` | Cancel and remove partial file |

Vault models remain in `{app_data}/models/` with the existing list/remove/verify/test commands.

## Environment

```bash
# Optional online registry merge (same JSON shape as models.json)
export AISEC_MODEL_REGISTRY_URL=https://example.com/aisec/models.json
```

## Implementation map

| Layer | Location |
|-------|----------|
| Registry loader | `crates/aisec-models/src/builtin_catalog.rs` |
| Download control | `crates/aisec-models/src/download/coordinator.rs` |
| ZIP import | `crates/aisec-models/src/import_pack.rs` |
| Tauri wiring | `src-tauri/src/model_registry.rs`, `commands/models.rs` |
| UI | `src/features/models/ModelsPage.tsx` |

## Tests

```bash
cargo test -p aisec-models
cargo test -p aisec-desktop --test models_commands
```

## Migration note

The deprecated `curated_catalog()` hardcoded list in `aisec-models` now returns empty. All catalog data must live in `resources/models.json` (plus optional remote merge).
