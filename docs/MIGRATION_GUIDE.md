# Migration Guide — Harness / Browser / Runtime Layer

## Overview

AISec attack execution now routes through the **Execution Harness** instead of direct HTTP transport. This is an internal refactor; existing targets and scans continue to work with updated wiring.

## Breaking changes

None for frontend IPC contracts. Target descriptor JSON format is unchanged.

## Backend changes

### New workspace members

```
crates/aisec-harness
crates/aisec-browser
crates/aisec-runtime
```

Add to dependent crates:

```toml
aisec-harness = { workspace = true, features = ["playwright"] }
```

### Attack transport

**Before**

```rust
SessionAwareTransport::Http(HttpTransport::new())
// or PlaywrightSessionTransport
```

**After**

```rust
build_attack_runtime_parts(...) // returns HarnessTargetTransport via HarnessFactory
```

### Judge input

**Before**

```rust
judge.judge(JudgeRequest { response_text: attempt.response.body, ... })
```

**After**

```rust
let normalized = NormalizedResponse::from_http(status, body, harness_kind);
judge.judge_normalized(probe_id, category, payload, &normalized).await?;
```

## Auth session storage

New canonical vault path: `{data_dir}/AuthSessions/` via `aisec-browser::auth_sessions_dir`.

Existing sessions under `{data_dir}/auth-vault/` remain valid. New recordings can migrate by re-saving sessions or continuing with existing `auth-vault` paths until re-recorded.

## Model registry

Offline catalog: `resources/models.json` (bundled). Optional remote update at startup — failure is non-fatal.

Model files remain under `{data_dir}/models/` with `registry.json`.

## Embedded runtime

Place platform Ollama binary at:

```
runtime/ollama        # macOS / Linux
runtime/ollama.exe    # Windows
```

`RuntimeSupervisor::ensure_running()` starts it automatically when present.

## Verification checklist

1. `cargo test -p aisec-harness -p aisec-browser -p aisec-runtime -p aisec-judge`
2. `cargo check -p aisec-desktop`
3. Scan wizard: Record Browser Session → Finish → run scan
4. Models page: install local judge model → set Judge mode to Local LLM
5. Settings → AI Models: select installed judge model

## Rollback

Revert `session_auth.rs` and `harness_runtime.rs` to use `SessionAwareTransport` + direct `judge.judge(JudgeRequest)`. Harness crates can remain in workspace without being linked from `aisec-desktop`.
