-- Session validation metadata for browser-authenticated targets

PRAGMA foreign_keys = ON;

ALTER TABLE auth_sessions ADD COLUMN validation_status TEXT NOT NULL DEFAULT 'valid';
ALTER TABLE auth_sessions ADD COLUMN last_validated_at TEXT;
ALTER TABLE auth_sessions ADD COLUMN user_identity TEXT;
