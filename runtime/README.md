# Embedded llama.cpp Runtime

AISec ships a platform-specific `llama-server` binary so local GGUF models work without Ollama.

## Layout

Place the llama.cpp server binary here before building or running:

```
runtime/
  llama-server        # macOS / Linux (chmod +x)
  llama-server.exe    # Windows
```

Development: `{repo_root}/runtime/llama-server`  
Release bundle: `{resource_dir}/runtime/llama-server` (see `src-tauri/tauri.conf.json`)

If no bundled binary exists, AISec falls back to `llama-server` on `PATH` when available.

## Model storage

GGUF models are stored under `{app_data}/models/` (AISec vault). Supported quantizations: **Q4, Q5, Q6, Q8** (detected from filename).

## Obtain binaries

Build or download `llama-server` from [llama.cpp](https://github.com/ggerganov/llama.cpp) for your target platform.

Do **not** commit large binaries to git; CI/dev machines place them locally or via your release pipeline.

## Environment

| Variable | Default | Purpose |
|----------|---------|---------|
| `AISEC_LLAMA_BASE_URL` | `http://127.0.0.1:8081` | llama-server HTTP API |
| `AISEC_LLAMA_HOST` | `127.0.0.1` | Bind host |
| `AISEC_LLAMA_PORT` | `8081` | HTTP port |
