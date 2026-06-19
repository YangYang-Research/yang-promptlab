-- Secure credential references (secrets live in OS keychain / encrypted vault only)

PRAGMA foreign_keys = ON;

ALTER TABLE auth_sessions ADD COLUMN credential_reference_id TEXT;
ALTER TABLE auth_profiles ADD COLUMN credential_reference_id TEXT;

CREATE INDEX IF NOT EXISTS idx_auth_sessions_credential_ref
    ON auth_sessions(credential_reference_id);

CREATE INDEX IF NOT EXISTS idx_auth_profiles_credential_ref
    ON auth_profiles(credential_reference_id);
