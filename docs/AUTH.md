# Authentication

**Last verified:** 2026-08-23

Wizard / target auth is **HTTP credentials on the Target Profile**, stored in `targets.descriptor_json` (sanitized). Verify and attack hydrate secrets in memory only.

Playwright login is **not selectable** in the wizard (`username_password` / `SSO` = disabled, “Temporarily unavailable”). Crate + IPC still compile.

```
Wizard: none | basic | api_key | jwt
  → sanitize_target_descriptor → OS keychain + credential_reference_id
  → SQLite descriptor without plaintext
  → resolve_descriptor_for_runtime / wizard (in-memory)
  → harness headers on verify + attack
```

IPC (credential path): target CRUD + `target_profile_verify*`. Diagnostic: `security_audit` / `security_migrate_secrets`.

SSOT for the endpoint itself: [DISCOVERY.md](DISCOVERY.md). Scan: [ATTACK.md](ATTACK.md).

---

## Product methods

| Kind | How it applies |
|------|----------------|
| `none` | No auth headers |
| `basic` | `Authorization: Basic …` |
| `api_key` | Named header + optional prefix |
| `jwt` | Bearer (or custom header/prefix) |

Headers may be inferred from Step 2 profile (`inferAuthFromProfileHeaders`) or cURL import. UI: wizard Auth/Verify (`TargetFormFields`).

---

## Secrets

| Secret | Storage |
|--------|---------|
| Target basic / JWT / API key | OS keychain; SQLite holds `credential_reference_id` |
| Third-party model / judge keys | Keychain scopes `model`, `judge` |
| Vault master key | Keychain `vault-key:master` |
| Playwright `storageState` (leftover) | AES-256-GCM `{session_id}.storage.enc` under `~/.promptlab/workspaces/AuthSessions/` |

Platform: `keyring` — Windows DPAPI, macOS Keychain, Linux Secret Service. Service: `com.promptlab.app`. Keys: `{scope}:{uuid}` (`target`, `profile`, `session`, `vault-key`, `model`, `judge`).

**Forbidden:** plaintext passwords/tokens in SQLite; plaintext `.storage.json` for new sessions; inline secrets in persisted descriptors.

Runtime: `resolve_descriptor_for_runtime` — never written back. Startup: `migrate_legacy_auth_data`, `migrate_legacy_target_descriptors`, `migrate_legacy_storage_artifacts`.

---

## Leftover Playwright (`promptlab-auth`)

Not a wizard feature. Runner: `crates/promptlab-auth/playwright/runner.mjs` (JSON-lines). IPC still registered: `auth_record_session_start` / `finish` / `cancel`, `auth_session_validate`, `auth_session_status`. UI panel exists behind the disabled radios.

Crate methods `username_password` / `oauth` / `oidc` / `saml` (browser) and JWT/API-key via `AuthEngine` / `TokenExtractor` remain for tests and replay. Dev: `npm run setup:playwright`. Release bundle: `npm run bundle:playwright` → `src-tauri/resources/playwright/` (gitignored).

```bash
cargo test -p promptlab-auth     # MockPlaywrightDriver + mock keyring
```
