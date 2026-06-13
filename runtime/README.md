# Embedded Ollama Runtime

AISec ships a platform-specific Ollama binary so local models work without a separate install.

## Layout

Place the official Ollama release binary here before building or running:

```
runtime/
  ollama        # macOS / Linux (chmod +x)
  ollama.exe    # Windows
```

Development: `{repo_root}/runtime/ollama`  
Release bundle: `{resource_dir}/runtime/ollama` (see `src-tauri/tauri.conf.json`)

If no bundled binary exists, AISec falls back to `ollama` on `PATH` when available.

## Model storage

Pulled models are stored under `{app_data}/models/` via `OLLAMA_MODELS`.

## Obtain binaries

Download from [https://ollama.com/download](https://ollama.com/download) for your target platform and copy into this directory.

Do **not** commit large binaries to git; CI/dev machines place them locally or via your release pipeline.
