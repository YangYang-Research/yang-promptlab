# Authentication

**Last verified:** 2026-08-25

Wizard / target auth is **HTTP credentials on the Target Profile**, stored in `targets.descriptor_json` (sanitized). Verify and attack hydrate secrets in memory only.

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

Browser login (username/password, SSO/OAuth) and the Playwright/Node release bundle were removed to shrink installers.

---

## Secrets

| Secret | Storage |
|--------|---------|
| Target basic / JWT / API key | OS keychain; SQLite holds `credential_reference_id` |
| Third-party model / judge keys | Keychain scopes `model`, `judge` |
| Vault master key | Keychain `vault-key:master` |

Platform: `keyring` — Windows DPAPI, macOS Keychain, Linux Secret Service. Service: `com.promptlab.app`. Keys: `{scope}:{uuid}` (`target`, `profile`, `session`, `vault-key`, `model`, `judge`).

**Forbidden:** plaintext passwords/tokens in SQLite; inline secrets in persisted descriptors.

Runtime: `resolve_descriptor_for_runtime` — never written back. Startup: `migrate_legacy_auth_data`, `migrate_legacy_target_descriptors`, `migrate_legacy_storage_artifacts`.
