# Auth Security Model

AISec uses a **single auth framework** (`aisec-auth`) for browser login recording, session replay, JWT/API-key auth, and target descriptor credentials. The unused `aisec-browser` crate was removed; vault paths live in `aisec-auth::paths`.

## Secret storage

| Secret type | Storage | SQLite column |
|-------------|---------|---------------|
| Profile password / JWT / API key | OS keychain (`keyring`) | `auth_profiles.credential_reference_id` + `*_credential_id` in `config_json` |
| Session cookies / bearer tokens | OS keychain | `auth_sessions.credential_reference_id` |
| Target descriptor secrets | OS keychain | `credential_reference_id` / `*_credential_id` in descriptor JSON |
| Playwright storageState files | AES-256-GCM on disk | `auth_sessions.storage_state_path` → `{session_id}.storage.enc` |
| Vault master key | OS keychain (`vault-key:master`) | *(not in SQLite)* |

Platform backends (via `keyring`):

- **Windows**: DPAPI
- **macOS**: Keychain
- **Linux**: Secret Service (libsecret)

Service name: `com.aisec.app`. Entry keys: `{scope}:{uuid}` where scope is `session`, `profile`, `target`, or `vault-key`.

## Forbidden

- Passwords, tokens, or session cookies in SQLite plaintext columns
- Plaintext Playwright `storageState` JSON on disk (new sessions write `.storage.enc` only)
- Inline secrets in persisted target descriptors after save (sanitized at `target_create`)

Runtime-only resolution: attack/discovery paths call `resolve_descriptor_for_runtime` to hydrate secrets from the keychain in memory; resolved values are never written back to SQLite.

## Vault layout

```
{app_data_dir}/
  AuthSessions/
    {session_id}.storage.enc    # encrypted Playwright storageState
  auth-vault/                   # legacy plaintext dir (read + migrate on startup)
```

Canonical path: `aisec_auth::auth_sessions_dir(data_dir)`.

## Database migration

Migration `006_auth_secure_credentials.sql` adds:

- `auth_sessions.credential_reference_id`
- `auth_profiles.credential_reference_id`

Legacy rows with `cookies_json` / `tokens_json` or inline config secrets are migrated on app startup via:

1. `migrate_legacy_auth_data` — moves session/profile secrets to keychain, clears SQLite columns
2. `migrate_legacy_target_descriptors` — strips inline secrets from target JSON
3. `migrate_legacy_storage_artifacts` — re-encrypts legacy `.storage.json` to `.storage.enc`

## API usage

```rust
use aisec_auth::{
    auth_sessions_dir, migrate_legacy_auth_data, SessionStore, SecretStore,
    sanitize_target_descriptor,
};

let store = SessionStore::new(db, auth_sessions_dir(&data_dir)).await?;
migrate_legacy_auth_data(&db, store.secrets()).await?;

// Persist target without inline secrets
let (sanitized, _) = sanitize_target_descriptor(&descriptor_json, store.secrets())?;
```

## Tests

- `aisec-auth`: `secrets/store`, `secrets/vault`, `secrets/descriptor`, `engine` (mock keyring)
- `aisec-storage`: auth repository CRUD with `credential_reference_id`

Tests use `keyring::mock::MockCredentialBuilder` — no real OS keychain required in CI.
