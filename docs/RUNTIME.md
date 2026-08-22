# Runtime and models

**Last verified:** 2026-08-22

Local inference is **in-process libllama** (`llama-cpp-2` FFI). No `llama-server`, no localhost HTTP for GGUF.

Product completions still go through the [harness](ARCHITECTURE.md#harness-ai-io). `promptlab-inference` picks **local GGUF** vs **third-party API** (`InferenceMode`) and records traffic / token usage (`runtime_traffic_*`, `runtime_token_usage`). Settings → Usage; AI Runtime page selects the route. Third-party keys: `models_save_third_party` + keychain (`ThirdPartyCredentialFields`). `models_test_embeddings` probes embedding endpoints.

```
Feature (judge | planner | generator | yazg | report | verify)
  → GatewaySession → AiInferenceGateway
      → local:  RuntimeManager → LocalRuntimeAdapter → LlamaInProcessRuntime → GGUF
      → remote: harness provider (OpenAI / Anthropic / Gemini / Bedrock / …)
```

FFI is confined to a worker thread; adapter access is a `tokio::sync::Mutex`. Missing GPU is non-fatal.

| Store | Path |
|-------|------|
| AI route | `~/.promptlab/config/ai_runtime_config.json` |
| Vault | `~/.promptlab/models/` |
| Hardware | `~/.promptlab/runtime/hardware.json` |
| Manifest | `~/.promptlab/runtime/manifest.json` |

`GfxBackend`: Auto | CUDA | Metal | Vulkan | CPU.

Startup (`lib.rs`): load inference config → `RuntimeManager::bootstrap()` (no model) → resume last GGUF if route is local → persist traffic/usage.

| Crate | Role |
|-------|------|
| `promptlab-inference` | Gateway, route, tokens, traffic |
| `promptlab-runtime` | libllama lifecycle, hardware |
| `promptlab-models` | Vault, catalog, downloads |
| `promptlab-harness` | Provider adapters |

UI: `/runtime`, `/models`. Judge weights: SQLite `judge_role_weights`. Also [runtime/README.md](../runtime/README.md).

---

## GGUF vault (`promptlab-models`)

Catalog SSOT: [`resources/models.json`](../resources/models.json). GGUF-first — no Ollama tags.

```
{vault}/models/{uuid}/model.gguf
{vault}/models/{uuid}/model.download.json
```

```json
{
  "id": "qwen3-8b-judge",
  "name": "Qwen3 8B Security Judge",
  "purpose": "judge",
  "engine": "llama.cpp",
  "format": "gguf",
  "download_url": "https://huggingface.co/.../model.q4_k_m.gguf"
}
```

Startup: bundled JSON → optional `PROMPTLAB_MODEL_REGISTRY_URL` merge → `validate_registry()` (`engine=llama.cpp`, HTTPS URL, unique `id`). Invalid entries are dropped from browse.

Downloads: HTTP `Range` resume + SHA256 stream. UI polls `models_download_status`. Import GGUF/ZIP via Tauri dialog.

IPC (see [ARCHITECTURE.md](ARCHITECTURE.md#ipc)): `models_browse`, `models_download_*`, `models_import_*`, `models_list/remove/verify/test_inference/test_embeddings`, `models_save_third_party`, `runtime_*` (including `install` / `repair` / `benchmark` / `traffic_stats` / `token_usage`).

```bash
cargo test -p promptlab-models
cargo test -p promptlab-runtime
```
