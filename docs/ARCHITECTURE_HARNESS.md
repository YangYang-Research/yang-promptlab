# PromptLab — Harness (AI I/O bus)

Harness is the **app-wide AI I/O layer**. Every completion goes through `HarnessFactory::execute`. New protocols register a provider; new features set `HarnessPurpose`. Analog: DeepSeek Harness `ctx.llm.stream` + `registerAdapter`.

## What goes through harness

| Caller | `HarnessPurpose` | Notes |
|--------|------------------|--------|
| Attack / scan | `attack` | Judge consumes `NormalizedResponse` |
| Wizard verify / capability probe | `verify` | Same descriptor + provider as scan |
| Discovery HTTP | `discover` | Crawl + API probes; surface `rest_api` |
| Fingerprint live probes | `fingerprint` | Reserved; offline fingerprint stays local |
| Assistant (Yazg) | `assistant` | Chat-native `messages[]` + tools |
| Judge / classifier / attacker LLM | `judge` | Same factory; vault auth |
| Wizard planner / endpoint classify | `wizard` | Token cap applied in factory policy |
| Attack planner | `planner` | |
| Prompt generator | `generator` | |
| Reports / summaries / recommend | `report` | |
| Connectivity / `test_chat` | `health` | |
| Future surfaces | `HarnessPurpose::named("…")` | No crate bump |

**Not harness:** HuggingFace downloads and llama-server **process** lifecycle (`promptlab-runtime`). Harness *calls* the runtime; feature crates do not open `reqwest` or `RemoteProviderAdapter` HTTP.

## Single execution path

```
Caller (attack | assistant | judge | …)
  → HarnessFactory::execute(descriptor, request)
    → purpose policy (token caps; not model-visible retries)
    → interceptors (attack plugins skip non-attack)
      → registered Harness (http | openai | anthropic | gemini | bedrock | llama | …)
        → NormalizedResponse (redacted, byte-capped)
```

Chat-native requests set `messages[]`, `model`, `max_tokens`, `tools`. Attack still maps `payload` → default probe body when those fields are empty.

## Extension

1. Add `crates/promptlab-harness/src/providers/<name>.rs` implementing `Harness`.
2. `registry.register` in `HarnessFactory::new` (or `factory.register` at runtime).
3. Map `TargetSurface` / `TargetProvider` → `HarnessKind`.
4. Callers keep using `execute()` — they do not grow HTTP clients.

`HarnessPurpose` is a string newtype. Product inference values: `assistant`, `judge`, `wizard`, `planner`, `generator`, `report`, `health`.

## Auth sessions

Browser sessions: `record_session` / `validate_session`. Playwright harness is registered per-scan on an **isolated** factory so it does not leak onto AppState.

Model vault credentials become `AuthMaterial` on the request (assistant/judge). Target descriptor auth is only for attack/verify/discover.

## Test doubles

`MockTransport` remains in `promptlab-attack` for unit tests. Harness integration tests use wiremock against `HarnessFactory::execute`.
