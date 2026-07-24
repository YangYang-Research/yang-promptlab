# Migration Guide — Harness / Browser / Runtime Layer

## Overview

PromptLab attack execution now routes through the **Execution Harness** instead of direct HTTP transport. This is an internal refactor; existing targets and scans continue to work with updated wiring.

## Breaking changes

None for frontend IPC contracts. Target descriptor JSON format is unchanged.

## Backend changes

### New workspace members

```
crates/promptlab-harness
crates/promptlab-browser
crates/promptlab-runtime
```

Add to dependent crates:

```toml
promptlab-harness = { workspace = true, features = ["playwright"] }
```

### Attack transport

**Before**

```rust
### Attack transport (post-consolidation)

```rust
// Production — always harness-backed
let runtime = build_attack_runtime_parts(...).await?;
let executor = attack_executor(runtime.transport); // HarnessTransport

// Library helper
let executor = default_executor_for("https://api.example.com/v1/chat/completions")?;
```

Do **not** instantiate HTTP transports directly. `MockTransport` is for tests only.
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

Canonical vault: `{data_dir}/AuthSessions/` via `promptlab_auth::auth_sessions_dir`.

Secrets (passwords, tokens, cookies) are stored in the OS keychain (`keyring`), referenced by `credential_reference_id` in SQLite. Playwright storageState files are AES-256-GCM encrypted as `{session_id}.storage.enc`.

On startup the desktop app runs backward-compatible migration:

1. `migrate_legacy_auth_data` — moves plaintext SQLite secrets to keychain
2. `migrate_legacy_target_descriptors` — strips inline secrets from target JSON
3. `migrate_legacy_storage_artifacts` — re-encrypts legacy `{auth-vault}/*.storage.json`

See [AUTH_SECURITY.md](./AUTH_SECURITY.md) for the full model.

## Model registry

Offline catalog: `resources/models.json` (bundled). Optional remote update at startup — failure is non-fatal.

Model files remain under `{data_dir}/models/` with `registry.json`.

## Embedded runtime

Place platform Ollama binary at:

```
runtime/ollama        # macOS / Linux
runtime/ollama.exe    # Windows
```

On startup the desktop app:

1. Resolves the binary (bundle → repo `runtime/` → system `PATH`)
2. Starts `ollama serve` with `OLLAMA_MODELS={app_data}/models`
3. Verifies health via `GET /api/tags`
4. Watches and auto-restarts on crash or failed health

IPC: `runtime_status`, `runtime_restart`, `runtime_stop`.

See [RUNTIME.md](./RUNTIME.md).

## Verification checklist

1. `cargo test -p promptlab-harness -p promptlab-browser -p promptlab-runtime -p promptlab-judge`
2. `cargo check -p promptlab-desktop`
3. Scan wizard: Record Browser Session → Finish → run scan
4. Models page: install local judge model → set Judge mode to Local LLM
5. Settings → AI Models: select installed judge model

## Rollback

Revert `session_auth.rs` and `harness_runtime.rs` to use `SessionAwareTransport` + direct `judge.judge(JudgeRequest)`. Harness crates can remain in workspace without being linked from `promptlab-desktop`.
