# AISec AI Inference Architecture

All LLM inference flows through **`aisec-inference`**. Local GGUF inference uses **embedded libllama** (in-process FFI) — no `llama-server`, no localhost HTTP.

## Layer diagram

```mermaid
flowchart TB
  subgraph Features
    J[Judge]
    P[Planner]
    G[Generator]
    M[Models]
  end

  subgraph Desktop
    IH[inference_host.rs]
    IRM[InferenceRuntimeManager]
    GS[GatewaySession]
  end

  subgraph Inference["aisec-inference"]
    GW[AiInferenceGateway]
    RA[LocalRuntimeAdapterBridge]
    PA[ProviderAdapter]
  end

  subgraph Runtime["aisec-runtime"]
    RM[RuntimeManager]
    RS[RuntimeSupervisor]
    LRA[LocalRuntimeAdapter]
  end

  subgraph Native["aisec-models / llama-cpp-2"]
    LIP[LlamaInProcessRuntime]
    LL[libllama FFI]
  end

  J & P & G & M --> IH --> GS --> GW --> IRM
  IRM --> RA --> RS --> LRA --> LIP --> LL
  IRM --> PA
```

## Local runtime path

```
AI Feature → AiInferenceGateway → InferenceRuntimeManager
  → RuntimeSupervisor → LocalRuntimeAdapter → LlamaInProcessRuntime → libllama → GGUF
```

No subprocess. No REST. No port binding.

## Configuration

| Store | Path |
|-------|------|
| AI route | `{data_dir}/ai_runtime_config.json` |
| Model vault | `{data_dir}/models/` |
| Hardware profile | `{data_dir}/runtime/hardware.json` |
| Runtime manifest | `{data_dir}/runtime/manifest.json` |

## Backend selection

`GfxBackend`: Auto | CUDA | Metal | Vulkan | CPU

Auto-detected from `RuntimeHardwareProfile` at startup. User override stored in runtime manifest.

## Startup

1. `InferenceRuntimeManager::load()` — AI route config
2. `RuntimeManager::bootstrap()` — hardware profile, init libllama (no model load)
3. `resume_local_runtime_on_startup()` — lazy-load selected GGUF if local mode

## Thread safety

`LlamaInProcessRuntime` confines all FFI to a dedicated worker thread. `LocalRuntimeAdapter` serializes access via `tokio::sync::Mutex`.
