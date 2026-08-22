# Target profile

**Last verified:** 2026-08-23

The desktop app does **not** crawl. Operators configure **one AI endpoint** as a **Target Profile** (`promptlab-target-profile`, `targets.profile_json`). That is the scan SSOT — URL, method, headers, `{{PROMPT}}` body template, provider, and capabilities.

```
Pick template or import cURL
  → Auth (API key / JWT / basic / Playwright session)
  → Verify
      1. Connect probe  (`Hello`) — harness only, no AI Runtime
      2. Capability probe (inventory prompt) — harness
      3. Yazg AnalyzeEndpoint classify — needs AI Runtime live
  → profile.verification + effective capabilities
  → planner / attack use the same template
```

IPC: `target_profile_list_templates`, `target_profile_save`, `target_profile_get`, `target_profile_verify` / `verify_connect` / `verify_capability` / `verify_ai` / `verify_ai_classify`.

Wizard: [ARCHITECTURE.md](ARCHITECTURE.md#scan-wizard). Plan/execute: [ATTACK.md](ATTACK.md).

---

## Templates (`target_profile_list_templates`)

| Provider | Typical path |
|----------|----------------|
| OpenAI-compatible | `/v1/chat/completions` |
| OpenRouter | `/chat/completions` |
| Anthropic | `/v1/messages` |
| Gemini / Azure OpenAI / Bedrock / GitHub Copilot | provider templates |
| OpenWebUI / Dify / Langflow | product HTTP shapes |
| MCP | JSON-RPC |
| Generic HTTP / WebSocket | blank template |

Import API (wizard): paste cURL → patch URL/method/headers/body onto the profile.

Default capabilities come from the provider (streaming, tools, conversation, attachments, memory, agent). Verify/Yazg may refine them. Planner reads **effective capabilities**, not a crawl fingerprint.

---

## Verify

| Probe | Prompt | Runtime |
|-------|--------|---------|
| Connect | `Hello` | Harness `purpose=verify` |
| Capability | Inventory prompt (`VERIFY_PROMPT`) | Same harness + auth headers |
| Classify | Captured HTTP snapshot | `YazgSupervisor::react_classify_probe` |

Failures persist on the profile (`verification` not verified) with a console row (method, URL, masked headers, status, latency, preview). Success sets `is_verified()` — required before `planner_generate_from_profile`.

Auth for probes: sanitized `descriptor_json` hydrated from keychain, or wizard inline headers. Playwright login is leftover (disabled in wizard) — [AUTH.md](AUTH.md).

---

## Legacy crawl (unused by the app)

These still compile; the wizard and `scan_start` (profile playbooks) do **not** call them:

| Piece | Status |
|-------|--------|
| `promptlab-discovery` | Library: examples + integration tests. No `discovery_run` IPC |
| `promptlab-fingerprint` | Rule engine; not run in the wizard |
| `promptlab-endpoint-metadata` | `AiEndpointMetadata` / `body_template_from_metadata` — only if a scan still has `endpoints` rows |
| `endpoints` table / `discovery-progress` | Schema + unused event name |
| Plugin type `discovery` | Manifest still valid; desktop crawler never invokes `discover` |

Do not document crawl BFS / OpenAPI path probes as product behavior.
