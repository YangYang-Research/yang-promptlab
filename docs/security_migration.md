# PromptLab Secret Migration

This document describes how PromptLab detects and migrates legacy plaintext secrets into secure storage.

## Goals

- Eliminate plaintext credentials in SQLite, JSON config files, and on-disk session artifacts.
- Route short-lived secrets to the **OS keychain** (macOS Keychain, Windows Credential Manager, Linux Secret Service).
- Route Playwright `storageState` blobs to the **encrypted vault** (AES-256-GCM under `auth-vault/`).

## Audited Areas

| Area | Storage | Legacy signal | Secure destination |
|------|---------|---------------|-------------------|
| **Targets** | `targets.descriptor_json` | Inline `password`, `token`, `key`, or `value` under `auth` | OS keychain (`SecretScope::Target`) + credential reference in descriptor |
| **Auth profiles** | `auth_profiles.config_json` | Top-level `password`, `token`, or `key` fields | OS keychain (`SecretScope::Profile`) + `*_credential_id` fields |
| **Auth sessions** | `auth_sessions.cookies_json` / `tokens_json` | Non-null JSON columns | OS keychain (`SecretScope::Session`) + `credential_reference_id` |
| **Session storage** | `auth_sessions.storage_state_path` | Path to `.storage.json` or non-`.enc` file | Encrypted vault (`*.storage.enc`) |
| **Judge config** | `{data_dir}/judge_config.json` | Non-empty `remote.api_key` | OS keychain (`SecretScope::Judge`) + `remote.api_key_credential_id` |

## Migration Flow

```
plaintext (DB / JSON file)
    → OS keychain (sessions, profiles, targets, judge API keys)
    → encrypted vault (Playwright storageState)
```

1. **Audit** — `security_audit` IPC scans all areas and returns counts plus per-record findings.
2. **Migrate** — `security_migrate_secrets` IPC runs migrations in order:
   - Session cookies/tokens and profile inline secrets (`migrate_legacy_auth_data`)
   - Target descriptor inline auth (`migrate_legacy_target_descriptors`)
   - Plaintext Playwright storage files (`migrate_legacy_storage_artifacts`)
   - Judge remote API key (`migrate_judge_config_secrets`)
3. **Re-audit** — Returns `auditBefore` and `auditAfter` in the migration report.

Startup also runs the same migrations automatically (best-effort) so existing installs upgrade on launch.

## Settings UI

**Settings → Security → Migrate Secrets**

- Shows legacy record counts when the Tauri backend is connected.
- **Migrate Secrets** runs the full migration pipeline and refreshes the audit summary.
- No workflow changes elsewhere; judge API keys saved via existing judge settings are sanitized on write.

## Legacy Detection Rules

### Targets

`sanitize_target_descriptor` / `descriptor_has_plaintext_secrets` inspect `auth.config` (and top-level auth fields) for non-empty `password`, `token`, `key`, or `value`.

### Auth profiles

`profile_config_has_plaintext` flags configs with non-empty top-level `password`, `token`, or `key`.

### Sessions

- Database: `cookies_json IS NOT NULL OR tokens_json IS NOT NULL` (see migration `006_auth_secure_credentials.sql`).
- Disk: `storage_state_path` does not end in `.enc`, or `{auth-vault}/{session_id}.storage.json` exists.

### Judge config

`judge_config_has_legacy_secrets` is true when `remote.api_key` is non-empty in `judge_config.json`. After migration, only `api_key_credential_id` is persisted; runtime resolves the key from the keychain when needed.

## IPC Reference

| Command | Purpose |
|---------|---------|
| `security_audit` | Return `SecretMigrationAudit` |
| `security_migrate_secrets` | Run migration; return `SecretMigrationReport` |

Rust integration tests: `src-tauri/tests/security_commands.rs`.

## Related Code

- `crates/promptlab-auth/src/secrets/audit.rs` — database + storage artifact audit
- `crates/promptlab-auth/src/secrets/migrate.rs` — session/profile/target/storage migrations
- `src-tauri/src/judge_config.rs` — judge API key sanitize/resolve/migrate
- `src-tauri/src/commands/security.rs` — IPC orchestration
- `crates/promptlab-storage/migrations/006_auth_secure_credentials.sql` — session credential reference column
