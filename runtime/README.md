# Embedded llama.cpp Runtime

AISec embeds a platform-specific `llama-server` binary for local GGUF inference.

## How it works

1. **Release builds** — `npm run bundle:llama` downloads `llama-server` into `runtime/` before packaging. Tauri bundles `runtime/` as app resources.
2. **First app launch** — the desktop shell copies the bundled binary into `{app_data}/runtime/llama-server` (or downloads release `b9551` from [llama.cpp](https://github.com/ggml-org/llama.cpp/releases) when no bundle exists).
3. **Startup** — AISec auto-starts `llama-server` and loads the newest verified GGUF from the model vault when available.

Development fallback order: bundled resources → `runtime/llama-server` in repo → system `PATH` → GitHub download.

## Layout

```
runtime/
  llama-server        # macOS / Linux (chmod +x)
  llama-server.exe    # Windows
```

Manual dev install (optional):

```bash
npm run bundle:llama
```

## Model storage

GGUF models live under `{app_data}/models/`. Supported quantizations: **Q4, Q5, Q6, Q8**.

## Environment

| Variable | Default | Purpose |
|----------|---------|---------|
| `AISEC_LLAMA_RELEASE` | `b9551` | llama.cpp GitHub release tag for auto-download |
| `AISEC_LLAMA_BASE_URL` | `http://127.0.0.1:8081` | llama-server HTTP API |
| `AISEC_LLAMA_HOST` | `127.0.0.1` | Bind host |
| `AISEC_LLAMA_PORT` | `8081` | HTTP port |

Do **not** commit large binaries to git; CI/release pipelines run `bundle:llama` locally or in the build job.
