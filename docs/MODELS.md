# Local Model Manager

**Crate:** `aisec-models`  
**Purpose:** GGUF model registry, HuggingFace downloads, SHA256 verification, hardware detection, llama.cpp runtime.

---

## Architecture

```
LocalModelManager
├── ModelRegistry        — in-memory GGUF catalog
├── DownloadManager      — HTTP + Range resume + HuggingFace URLs
├── VerificationEngine   — streaming SHA256
├── HardwareProfile      — CPU, RAM, GPU (CUDA/Metal)
└── LlamaCppRuntime      — llama-server subprocess + /completion API
```

---

## Model Registry

```rust
use aisec_models::LocalModelManager;

let mut mgr = LocalModelManager::new("./data/models")?;
let entry = mgr.import_local("llama-3-8b", "/path/to/model.gguf")?;
let models = mgr.list_models();
```

Vault layout:

```
{vault}/
  models/{uuid}/model.gguf
  models/{uuid}/model.download.json   # resume state (during download)
```

---

## HuggingFace Downloads

```rust
use aisec_models::HuggingFaceDownloadRequest;

let entry = mgr.download_huggingface(HuggingFaceDownloadRequest {
    name: "Llama 3 8B Q4".into(),
    repo: "QuantFactory/Meta-Llama-3-8B-GGUF".into(),
    filename: "Meta-Llama-3-8B.Q4_K_M.gguf".into(),
    revision: Some("main".into()),
    expected_sha256: Some("abc123...".into()),
    expected_size_bytes: None,
}).await?;
```

URL format: `https://huggingface.co/{repo}/resolve/{revision}/{filename}`

Resume: partial file + `.download.json` sidecar → `Range: bytes=N-` on retry.

---

## Verification

```rust
use aisec_models::VerificationEngine;

let (hash, size) = VerificationEngine::hash_file("model.gguf").await?;
let result = VerificationEngine::verify_or_fail("model.gguf", &expected).await?;
```

Streaming 1 MiB chunks — suitable for multi-GB GGUF files.

---

## Hardware / GPU Detection

```rust
use aisec_models::detect_hardware;

let hw = detect_hardware()?;
println!("{} cores, {} GB RAM, {} GPU(s)",
    hw.cpu_cores,
    hw.total_memory_bytes / (1024*1024*1024),
    hw.gpus.len());
```

| Platform | GPU detection |
|----------|---------------|
| macOS | `system_profiler` + Apple Silicon Metal fallback |
| Linux | `nvidia-smi` → CUDA |
| All | CPU cores via `available_parallelism`, RAM via sysctl/`/proc/meminfo` |

`recommended_gpu_layers()` auto-configures llama.cpp `-ngl`.

---

## llama.cpp Runtime

Requires `llama-server` on PATH or configured binary:

```rust
use aisec_models::{InferenceRequest, LlamaCppConfig, LlamaCppRuntime};
use aisec_models::runtime::InferenceRuntime;

let mut runtime = LlamaCppRuntime::new(LlamaCppConfig::default());
runtime.load_model(path_to_gguf).await?;
let resp = runtime.complete(InferenceRequest {
    prompt: "Analyze this prompt injection...".into(),
    max_tokens: 256,
    temperature: 0.7,
}).await?;
```

API: `POST /completion` (llama.cpp server), health via `GET /health`.

---

## Tests

```bash
cargo test -p aisec-models
```

Uses `wiremock` for download/resume tests and `MockInferenceRuntime` for inference without llama.cpp.

---

## Environment

| Variable | Default | Purpose |
|----------|---------|---------|
| `AISEC_MODEL_VAULT` | `./data/models` | Model storage directory |
