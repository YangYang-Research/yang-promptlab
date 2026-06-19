# Embedded Runtime

AISec runs local LLM inference through an embedded **Ollama** process managed by `aisec-runtime::RuntimeSupervisor`.

## Binary layout

| Platform | Path |
|----------|------|
| Windows | `runtime/ollama.exe` |
| macOS | `runtime/ollama` |
| Linux | `runtime/ollama` |

Resolution order at startup:

1. Tauri resource bundle `{resource_dir}/runtime/ollama`
2. Repo dev path `{repo_root}/runtime/ollama`
3. System `PATH` (`which ollama`)

## Startup lifecycle

On desktop launch (`src-tauri/src/lib.rs`):

1. **Detect** — resolve binary via `ollama_runtime::resolve_runtime_config`
2. **Start** — `RuntimeSupervisor::ensure_running()` spawns `ollama serve`
3. **Verify** — poll `GET /api/tags` until healthy (or fail gracefully)
4. **Watch** — background task restarts on process exit or failed health check

Environment:

- `OLLAMA_MODELS` → `{app_data}/models`
- `OLLAMA_HOST` → optional override (default `http://127.0.0.1:11434`)

## IPC commands

| Command | Description |
|---------|-------------|
| `runtime_status` | State, binary path, health, installed Ollama models |
| `runtime_restart` | Stop and start embedded runtime |
| `runtime_stop` | Stop embedded runtime |

### `runtime_status` response

```json
{
  "state": "running",
  "binaryPath": "/path/to/ollama",
  "binaryAvailable": true,
  "baseUrl": "http://127.0.0.1:11434",
  "healthy": true,
  "installedModels": [{ "name": "llama3:latest", "sizeBytes": 4661211424 }],
  "message": "embedded runtime is running and healthy"
}
```

## Model discovery

`RuntimeSupervisor::list_installed_models()` calls Ollama `GET /api/tags`. Results appear in `runtime_status.installedModels`.

Vault models (`models_list`) and Ollama tags are complementary:

- **Vault** — AISec registry + GGUF/Ollama install tracking under `{app_data}/models/`
- **Ollama tags** — live list from the running embedded server

`models_install` defaults to the supervisor's `baseUrl` when the client omits `ollamaBaseUrl`.

## Graceful shutdown

On app exit, `RuntimeSupervisor::stop()` kills the child process before the SQLite pool closes.

## Tests

```bash
cargo test -p aisec-runtime
cargo check -p aisec-desktop
```

## Related

- `docs/MIGRATION_GUIDE.md` — migration notes
- `runtime/README.md` — binary placement for release builds
