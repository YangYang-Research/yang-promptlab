-- Authentication profiles, sessions, and login recordings

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS auth_profiles (
    id          TEXT PRIMARY KEY NOT NULL,
    project_id  TEXT REFERENCES projects(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    method      TEXT NOT NULL,
    config_json TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_auth_profiles_project_id ON auth_profiles(project_id);
CREATE INDEX IF NOT EXISTS idx_auth_profiles_method ON auth_profiles(method);

CREATE TABLE IF NOT EXISTS auth_sessions (
    id                 TEXT PRIMARY KEY NOT NULL,
    profile_id         TEXT NOT NULL REFERENCES auth_profiles(id) ON DELETE CASCADE,
    status             TEXT NOT NULL DEFAULT 'active',
    cookies_json       TEXT,
    tokens_json        TEXT,
    storage_state_path TEXT,
    expires_at         TEXT,
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_auth_sessions_profile_id ON auth_sessions(profile_id);
CREATE INDEX IF NOT EXISTS idx_auth_sessions_status ON auth_sessions(status);

CREATE TABLE IF NOT EXISTS auth_recordings (
    id                 TEXT PRIMARY KEY NOT NULL,
    profile_id         TEXT NOT NULL REFERENCES auth_profiles(id) ON DELETE CASCADE,
    steps_json         TEXT NOT NULL,
    storage_state_path TEXT,
    metadata_json      TEXT,
    created_at         TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_auth_recordings_profile_id ON auth_recordings(profile_id);
