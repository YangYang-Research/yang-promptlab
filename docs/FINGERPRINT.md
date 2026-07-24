# AI Endpoint Fingerprinting

**Crate:** `promptlab-fingerprint`  
**Purpose:** Identify AI inference providers from HTTP observations with weighted confidence scoring.

---

## Supported Providers

| Provider | Key signals |
|----------|-------------|
| **OpenAI** | `api.openai.com`, `/v1/chat/completions`, `OpenAI-Organization` header, models list |
| **Anthropic** | `api.anthropic.com`, `/v1/messages`, `anthropic-version` header |
| **Gemini** | `generativelanguage.googleapis.com`, `:generateContent`, `candidates` body |
| **Bedrock** | `bedrock-runtime.*.amazonaws.com`, `/model/*/invoke`, `x-amzn-requestid` |
| **Azure OpenAI** | `*.openai.azure.com`, `/openai/deployments/*/chat/completions` |
| **Ollama** | `/api/tags`, `/api/chat`, port `11434`, `done_reason` field |
| **LiteLLM** | `x-litellm-*` headers, `litellm_error` type |
| **vLLM** | `Server: vllm`, `/health` + `"status":"ok"`, metrics path |

---

## Detection Rules

Rules live in `crates/promptlab-fingerprint/src/rules/providers/`. Each rule has:

- **id** — stable identifier (e.g. `openai.host`)
- **weight** — contribution to raw score (0.15–0.50)
- **matcher** — host, path regex, header, JSON body, status code

Full catalog: **59 rules** across 8 providers (see `rule_catalog()`).

---

## Confidence Scoring

```
raw_score = Σ matched rule weights
confidence = 1 - e^(-raw_score) + diversity_bonus
```

| Factor | Effect |
|--------|--------|
| Multiple signal types | +0.04 per extra type (max +0.12) |
| Single strong signal (≥0.40 weight) | floor 0.72 |
| Generic OpenAI path only | −25% when multiple compat servers match |
| Azure host + OpenAI path | OpenAI score ×0.5 |
| LiteLLM headers | Other compat providers ×0.7 |
| vLLM `Server` header | OpenAI score ×0.6 |

Default threshold: **0.45** (below = excluded from results)

---

## Usage

```rust
use promptlab_fingerprint::{FingerprintEngine, FingerprintInput};
use std::collections::HashMap;

let engine = FingerprintEngine::new();
let input = FingerprintInput {
    url: "https://api.anthropic.com/v1/messages".into(),
    method: Some("POST".into()),
    status: Some(401),
    headers: HashMap::from([
        ("anthropic-version".into(), "2023-06-01".into()),
    ]),
    body: Some(r#"{"error":{"type":"authentication_error"}}"#.into()),
};

let report = engine.fingerprint(&input);
if let Some(fp) = report.best_match() {
    println!("{} {:.0}%", fp.provider.display_name(), fp.confidence * 100.0);
}
```

---

## Tests

```bash
cargo test -p promptlab-fingerprint
```

Per-provider integration tests in `engine.rs` cover all 8 targets.

---

## Integration

Use with `promptlab-discovery` HTTP snapshots by building `FingerprintInput` from URL, status, headers, and body after probe/crawl.
