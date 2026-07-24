# PromptLab Fingerprint Engine

The fingerprint engine identifies AI platforms and deployment components **before attack execution**, enabling targeted scan planning in the Scan Wizard.

## Pipeline

```
Discovery  →  Fingerprint  →  Attack Planning
```

| Stage | Crate / module | What happens |
|-------|----------------|--------------|
| **Discovery** | `promptlab-discovery` | Crawl target, probe routes, enumerate endpoints |
| **Fingerprint** | `promptlab-fingerprint` | HTTP probe + rule evaluation per endpoint |
| **Attack planning** | Scan Wizard step 4 | Suggest categories from fingerprint recommendations |

Discovery runs in `discovery_run` (`src-tauri/src/commands/discovery.rs`). Each eligible endpoint is probed via `fingerprint_service::fingerprint_endpoint_url`, which calls `FingerprintEngine::fingerprint_stack`.

Eligible endpoint kinds: `ai_endpoint`, `openapi`, `graphql`, `javascript`, `rest_api`, plus manual and plugin endpoints (fingerprinted on create).

## Crate layout

```
crates/promptlab-fingerprint/
├── src/
│   ├── engine.rs          # FingerprintEngine orchestration
│   ├── profile.rs         # PlatformProfile builder (attack-planning output)
│   ├── evaluator.rs       # Rule matchers (path, header, body, JSON)
│   ├── scoring.rs         # Confidence + conflict penalties
│   ├── recommendations.rs # Attack category suggestions
│   ├── openapi.rs         # OpenAPI spec → synthetic inputs
│   ├── types.rs           # Providers, frameworks, components, reports
│   └── rules/
│       ├── providers/     # OpenAI, Anthropic, Azure, Ollama, …
│       └── stack.rs       # Framework + component rules
└── examples/detect_ai.rs  # CLI demo
```

## Platform profile output

Every `StackFingerprintReport` includes a normalized `platform_profile` used for attack planning:

```json
{
  "platform": "dify",
  "version": "",
  "auth_type": "api_key",
  "llm_provider": "openai",
  "memory_enabled": true,
  "tools_enabled": true,
  "rag_enabled": true
}
```

| Field | Source |
|-------|--------|
| `platform` | Top agent framework, else `mcp_server`, else `{provider}_api` |
| `version` | Response JSON `version` / headers when present |
| `auth_type` | `bearer`, `api_key`, `basic`, `required`, `none`, … |
| `llm_provider` | Primary inference provider (`openai`, `anthropic`, `ollama`, …) |
| `memory_enabled` | Chat platforms, conversation/thread routes, memory fields |
| `tools_enabled` | MCP, tool orchestration, agent frameworks |
| `rag_enabled` | RAG component, knowledge-base platforms, retrieval signals |

IPC exposes this as `platformProfile` (camelCase) on `EndpointFingerprintDto`.

## Detected platforms

### Agent / UI frameworks

| Platform | Rule signals |
|----------|--------------|
| **OpenWebUI** | `/api/v1/chats`, branding, JS assets |
| **Dify** | `/v1/chat-messages`, body references |
| **Flowise** | `/api/v1/prediction`, branding |
| **Langflow** | `/api/v1/run`, `/api/v1/flows`, branding |
| **LibreChat** | `/api/ask`, `/api/messages`, branding |
| LangChain, LangGraph, LangServe, AnythingLLM, CrewAI, AutoGen | See `rules/stack.rs` |

### Inference APIs

| Platform ID | Provider rules |
|-------------|----------------|
| `openai_api` | OpenAI |
| `anthropic_api` | Anthropic |
| `azure_openai_api` | Azure OpenAI |
| `ollama_api` | Ollama |
| Also: Gemini, Bedrock, LiteLLM, vLLM, OpenRouter | `rules/providers/` |

### Components

| Component | Signals |
|-----------|---------|
| **MCP Servers** | `/mcp` route, JSON-RPC `tools/list`, SSE |
| RAG pipeline | `/retrieve`, `source_documents`, knowledge-base refs |
| Tool orchestration | `/tools`, `tools` array in responses |

## Attack recommendations

`generate_attack_recommendations()` maps detections to attack categories:

| Fingerprint category | Scan Wizard category |
|---------------------|----------------------|
| `prompt_injection` | `prompt_injection` |
| `jailbreak` | `jailbreak` |
| `system_prompt_leakage` | `system_prompt_extraction` |
| `rag_leakage` | `rag_leakage` |
| `tool_abuse` | `tool_abuse` |
| `mcp_abuse` | `mcp_abuse` |

Categories like `data_exfiltration` and `policy_bypass` are emitted for provider-specific cases but are not yet mapped to attack engine categories.

## Scan Wizard integration

### Step 3 — Discovery

- Discovery phases include **Fingerprint** (animated with crawl/API/OpenAPI phases).
- Endpoint table shows a **Platform** column from `platformProfile`.
- Summary line lists detected platforms for selected endpoints.

### Step 4 — Attack Planning

- **Fingerprint summary** card shows platform capabilities (memory, tools, RAG).
- **Apply suggestions** pre-selects attack categories from fingerprint recommendations across selected endpoints.

### Data flow

```
discovery_run
  → fingerprint_endpoint_url (per endpoint)
  → StackFingerprintReport (stored as endpoints.fingerprint_json)
  → EndpointDto.fingerprint.platformProfile
  → fingerprintPlan.ts (aggregate suggestions)
  → AttackPlanStep
```

## Usage (Rust)

```rust
use promptlab_fingerprint::{FingerprintEngine, FingerprintInput};

let engine = FingerprintEngine::new();
let input = FingerprintInput::from_snapshot(
    "https://target.example/api/v1/chat-messages",
    Some("POST".into()),
    401,
    headers,
    Some("application/json".into()),
    body,
    Some("rest_api".into()),
);

let report = engine.fingerprint_stack(&input);
println!("platform: {}", report.platform_profile.platform);
println!("recommendations: {:?}", report.attack_recommendations);
```

## Related files

| Area | Path |
|------|------|
| Engine | `crates/promptlab-fingerprint/src/engine.rs` |
| Platform profile | `crates/promptlab-fingerprint/src/profile.rs` |
| Tauri service | `src-tauri/src/fingerprint_service.rs` |
| Discovery IPC | `src-tauri/src/commands/discovery.rs` |
| Wizard UI | `src/features/scans/steps/DiscoveryStep.tsx`, `AttackPlanStep.tsx` |
| Category mapping | `src/features/scans/fingerprintPlan.ts` |
