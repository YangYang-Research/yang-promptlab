# Authentication

**Crate:** `promptlab-auth`  
**Browser:** Playwright Chromium (Node JSON-lines runner)

Records, stores, and replays authenticated sessions for target probes. Single framework for browser login, JWT/API-key, and target-descriptor credentials.

**Last verified:** 2026-08-22

---

## Architecture

```
Tauri IPC
  → AuthEngine (record / replay / extract)
      → PlaywrightClient → runner.mjs → Chromium
      → TokenExtractor (JWT / API key — no browser)
      → SessionStore → SQLite metadata + encrypted vault
      → SecretStore  → OS keychain
```

| Method | Browser | Notes |
|--------|---------|-------|
| Username/password | Yes | Form fill or interactive |
| OAuth / OIDC / SAML | Yes | Interactive URL wait |
| JWT | No | Configured token |
| API key | No | Header + key |

---

## Playwright

JSON-lines stdin/stdout to `crates/promptlab-auth/playwright/runner.mjs`:

| Command | Purpose |
|---------|---------|
| `launch` | Chromium + optional `storageState` |
| `record_login` | Navigate + login |
| `replay_session` | Restore cookies/storage and open URL |
| `extract_tokens` | Headers, JSON bodies, web storage |
| `get_cookies` / `set_cookies` / `close` | Jar + teardown |

Dev: `npm run setup:playwright`. Release: `npm run bundle:playwright` → `src-tauri/resources/playwright/` (gitignored).

IPC: `auth_record_session_start` / `finish` / `cancel`, `auth_session_validate`, `auth_session_status`. Wizard auth step embeds `PlaywrightRecordPanel` (same commands). Diagnostic: `security_audit` / `security_migrate_secrets`.

---

## Secret storage

| Secret | Storage |
|--------|---------|
| Profile password / JWT / API key | OS keychain; SQLite holds `credential_reference_id` |
| Session cookies / bearer | OS keychain |
| Target descriptor secrets | OS keychain; JSON sanitized at save |
| Playwright `storageState` | AES-256-GCM file `{session_id}.storage.enc` |
| Vault master key | Keychain `vault-key:master` |

Platform backends (`keyring`): Windows DPAPI, macOS Keychain, Linux Secret Service. Service: `com.promptlab.app`. Keys: `{scope}:{uuid}` (`session`, `profile`, `target`, `vault-key`).

**Forbidden:** plaintext passwords/tokens/cookies in SQLite; plaintext `.storage.json` for new sessions; inline secrets in persisted descriptors.

Runtime hydration: `resolve_descriptor_for_runtime` — in-memory only, never written back.

Vault path: `~/.promptlab/workspaces/AuthSessions/` (`promptlab_auth::auth_sessions_dir`).

Startup: `migrate_legacy_auth_data`, `migrate_legacy_target_descriptors`, `migrate_legacy_storage_artifacts`.

---

## Tests

```bash
cargo test -p promptlab-auth     # MockPlaywrightDriver + mock keyring
```

Schema: [ARCHITECTURE.md](ARCHITECTURE.md#sqlite-promptlab-storage).
