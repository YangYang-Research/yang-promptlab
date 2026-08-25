# Runtime and models

**Last verified:** 2026-08-25

AI Runtime is **remote-only**. PromptLab talks to third-party HTTP providers (OpenAI, Anthropic, Gemini, Azure, Bedrock, OpenRouter, custom OpenAI-compatible including Ollama over HTTP). There is **no** embedded llama.cpp / in-process GGUF runtime.

Product completions go through the [harness](ARCHITECTURE.md#harness-ai-io). `promptlab-inference` routes to remote providers (`InferenceMode::ThirdParty`) and records traffic / token usage (`runtime_traffic_*`, `runtime_token_usage`). Settings → Usage; AI Runtime page selects the active remote model. Third-party keys: `models_save_third_party` + keychain (`ThirdPartyCredentialFields`).

```
Feature (judge | planner | generator | yazg | report | verify)
  → GatewaySession → AiInferenceGateway
      → remote: harness provider (OpenAI / Anthropic / Gemini / Bedrock / Ollama HTTP / …)
```

| Store | Path |
|-------|------|
| AI route | `~/.promptlab/config/ai_runtime_config.json` |
| Vault | `~/.promptlab/models/` (remote registry metadata) |

Startup (`lib.rs`): load inference config → optional connectivity check → persist traffic/usage. No GGUF preload.

| Crate | Role |
|-------|------|
| `promptlab-inference` | Gateway, route, tokens, traffic |
| `promptlab-runtime` | Remote-oriented host stubs / SharedModelProvider |
| `promptlab-models` | Vault registry, third-party entries |
| `promptlab-harness` | Provider adapters |

UI: `/runtime`, `/models` (third-party panel only). No built-in GGUF `models.json` catalog. Judge weights: SQLite `judge_role_weights`.

IPC: `models_list/remove/verify/test_*`, `models_save_third_party`, `runtime_status`, `runtime_set_inference_route` (third_party only), `runtime_test_*`, `runtime_traffic_*`, `runtime_token_usage`, judge weights.

```bash
cargo test -p promptlab-models
cargo test -p promptlab-runtime
```
