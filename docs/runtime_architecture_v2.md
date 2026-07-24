# PromptLab Runtime Architecture v2 — Embedded llama.cpp

**Version:** 2.0  
**Date:** 2026-06-13  
**Replaces:** Ollama-embedded runtime (v1)

---

## Overview

PromptLab Desktop runs local LLM inference through an embedded **llama.cpp** server (`llama-server`) supervised by `promptlab-runtime::RuntimeSupervisor`. Models are **GGUF files** in the app vault (`{app_data}/models/`). The Judge Engine consumes inference exclusively through the **ModelProvider** abstraction — no direct runtime coupling in `promptlab-judge`.

---

## Layer Diagram

```mermaid
flowchart TB
    subgraph UI["Frontend (unchanged)"]
        MP[Models Page]
        JP[Judge Settings]
    end

    subgraph Tauri["src-tauri IPC"]
        RC[runtime_status / restart / stop]
        MC[models_* commands]
        JC[judge_config + attack/scan]
    end

    subgraph Judge["promptlab-judge"]
        JE[JudgeEngine]
        MPR[ModelProviderRuntime]
    end

    subgraph Runtime["promptlab-runtime"]
        EMP[EmbeddedModelProvider]
        RS[RuntimeSupervisor]
        LCR[LlamaCppRuntime]
    end

    subgraph Models["promptlab-models"]
        LMM[LocalModelManager]
        LIE[LocalInferenceEngine]
        VAULT[(GGUF Vault)]
    end

    MP --> MC
    JP --> JC
    JC --> JE
    JE --> MPR
    MPR --> EMP
    EMP --> LMM
    LMM --> LIE
    LIE --> VAULT
    RC --> RS
    RS --> LCR
    LCR --> VAULT
    MC --> LMM
```

---

## Startup Flow

```mermaid
sequenceDiagram
    participant App as Tauri lib.rs
    participant ER as embedded_runtime
    participant RS as RuntimeSupervisor
    participant MR as model_registry
    participant LMM as LocalModelManager

    App->>ER: resolve_runtime_config()
    Note over ER: bundle → repo/runtime → PATH
    ER-->>App: RuntimeConfig + llama-server path
    App->>ER: start_embedded_runtime(config)
    ER->>RS: ensure_running()
    alt binary missing
        RS-->>App: Unavailable (idle app, no crash)
    else binary present
        RS->>RS: scan vault for GGUF
        RS-->>App: Running (idle-ready)
    end
    App->>MR: open_model_manager_with_registry()
    MR-->>App: LocalModelManager
    App->>LMM: with_llama_binary(path)
    App->>App: EmbeddedModelProvider::new(manager)
    App->>App: spawn runtime_watch (if started)
```

### Startup steps

1. Resolve `llama-server` binary (`embedded_runtime.rs`).
2. Create `RuntimeSupervisor` with `RuntimeConfig` (host/port/base URL).
3. `ensure_running()` — create models dir; enter **idle-ready** if binary exists.
4. Load `resources/models.json` into `LocalModelManager`.
5. Wire `EmbeddedModelProvider` → shared `Arc<Mutex<LocalModelManager>>`.
6. Optional background `runtime_watch` restarts unhealthy loaded servers.

---

## Model Loading Flow

```mermaid
sequenceDiagram
    participant UI as Models Page
    participant IPC as models_install / import
    participant LMM as LocalModelManager
    participant DL as DownloadCoordinator
    participant VAULT as GGUF Vault
    participant RS as RuntimeSupervisor
    participant LCR as LlamaCppRuntime

    UI->>IPC: models_download_start / import_gguf
    IPC->>LMM: install_catalog / import_local
    alt HuggingFace GGUF
        LMM->>DL: download to vault
        DL-->>VAULT: *.gguf (Q4/Q5/Q6/Q8)
    else Local import
        LMM->>VAULT: copy / extract GGUF
    end
    LMM-->>UI: ModelEntryDto

    Note over RS,LCR: On-demand for Judge
    UI->>IPC: scan / attack (local judge)
    IPC->>RS: ensure_model_loaded(gguf_path)
    RS->>LCR: load_model(path)
    LCR->>LCR: validate .gguf + detect quant
    LCR->>LCR: spawn llama-server -m path
    LCR->>LCR: poll GET /health
    LCR-->>RS: Ready
```

### GGUF quantization support

| Quantization | Filename patterns (examples) |
|--------------|------------------------------|
| Q4 | `q4_k_m`, `.q4`, `Q4_K` |
| Q5 | `q5_k_s`, `.q5`, `Q5_K` |
| Q6 | `q6_k`, `.q6` |
| Q8 | `q8_0`, `.q8`, `Q8_K` |

Detection: `promptlab-runtime::runtime::gguf::detect_quantization()`.

---

## Inference Flow (Judge)

```mermaid
sequenceDiagram
    participant Attack as attack.rs / scan.rs
    participant JC as judge_config
    participant JE as JudgeEngine
    participant MPR as ModelProviderRuntime
    participant EMP as EmbeddedModelProvider
    participant LMM as LocalModelManager
    participant LIE as LocalInferenceEngine
    participant LCR as LlamaCppRuntime (models)

    Attack->>JC: build_configured_judge_engine()
    JC->>JC: prepare_judge_runtime_context()
    JC->>JC: ensure_running() + ensure_model_loaded()
    JC->>JE: build_judge_engine(config, context)
    Attack->>JE: judge_normalized(...)
    JE->>MPR: complete(prompt)
    MPR->>EMP: complete_for_model(vault_id)
    EMP->>LMM: inference_engine(vault_id)
    LMM->>LIE: from_entry(GGUF)
    LIE->>LCR: load_model + complete()
    LCR->>LCR: POST /completion
    LCR-->>JE: InferenceResponse
    JE-->>Attack: JudgeVerdict
```

**Judge API unchanged:** `judge_normalized`, `JudgeProviderConfig`, `JudgeRuntimeContext` signatures are identical to v1.

---

## RuntimeSupervisor State Machine

```mermaid
stateDiagram-v2
    [*] --> Stopped
    Stopped --> Starting: ensure_model_loaded()
    Starting --> Running: health OK
    Starting --> Failed: timeout / spawn error
    Running --> Stopped: stop()
    Running --> Starting: restart() / unhealthy
    Failed --> Stopped: stop()
    Stopped --> Running: ensure_running() idle-ready
```

| State | Meaning |
|-------|---------|
| **Stopped** | No `llama-server` process |
| **Starting** | Spawning server for GGUF |
| **Running** | Idle-ready (binary OK) or server healthy |
| **Failed** | Startup timeout or spawn error |

---

## IPC Surface (unchanged)

| Command | Purpose |
|---------|---------|
| `runtime_status` | Binary path, health, GGUF list |
| `runtime_restart` | Stop + reload active model |
| `runtime_stop` | Shutdown server |
| `models_*` | Vault browse/install/import/download |
| `judge_*` | Config + connectivity tests |

DTO field names (`ollamaTag`, `ollamaBaseUrl`, `installedModels`) preserved for frontend compatibility.

---

## Binary & Config

| Item | Value |
|------|-------|
| Binary | `runtime/llama-server` |
| Default URL | `http://127.0.0.1:8081` |
| Vault | `{app_data}/models/` |
| Env | `PROMPTLAB_LLAMA_BASE_URL`, `PROMPTLAB_LLAMA_HOST`, `PROMPTLAB_LLAMA_PORT` |

---

## Crate Responsibilities

| Crate | Responsibility |
|-------|----------------|
| `promptlab-runtime` | Supervisor, embedded `LlamaCppRuntime`, `EmbeddedModelProvider`, GGUF discovery |
| `promptlab-models` | Vault registry, downloads, per-model `LocalInferenceEngine` |
| `promptlab-judge` | Verdict logic; `ModelProviderRuntime` bridge only |
| `src-tauri` | IPC, startup wiring, judge_config orchestration |

---

## Related Documents

- [runtime_migration_report.md](./runtime_migration_report.md) — Ollama audit and touchpoint map
- [JUDGE_RUNTIME.md](./JUDGE_RUNTIME.md) — Judge ↔ ModelProvider integration
- [MODEL_REGISTRY.md](./MODEL_REGISTRY.md) — Catalog and download IPC
