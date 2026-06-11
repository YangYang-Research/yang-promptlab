-- AISec initial schema

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS projects (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    description TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS targets (
    id              TEXT PRIMARY KEY NOT NULL,
    project_id      TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    target_type     TEXT NOT NULL,
    descriptor_json TEXT NOT NULL DEFAULT '{}',
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_targets_project_id ON targets(project_id);

CREATE TABLE IF NOT EXISTS scans (
    id            TEXT PRIMARY KEY NOT NULL,
    project_id    TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    target_id     TEXT REFERENCES targets(id) ON DELETE SET NULL,
    name          TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'pending',
    playbook_json TEXT,
    started_at    TEXT,
    completed_at  TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_scans_project_id ON scans(project_id);
CREATE INDEX IF NOT EXISTS idx_scans_target_id ON scans(target_id);
CREATE INDEX IF NOT EXISTS idx_scans_status ON scans(status);

CREATE TABLE IF NOT EXISTS findings (
    id             TEXT PRIMARY KEY NOT NULL,
    scan_id        TEXT NOT NULL REFERENCES scans(id) ON DELETE CASCADE,
    project_id     TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    target_id      TEXT REFERENCES targets(id) ON DELETE SET NULL,
    title          TEXT NOT NULL,
    severity       TEXT NOT NULL,
    category       TEXT,
    description    TEXT,
    evidence_json  TEXT,
    status         TEXT NOT NULL DEFAULT 'open',
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_findings_scan_id ON findings(scan_id);
CREATE INDEX IF NOT EXISTS idx_findings_project_id ON findings(project_id);
CREATE INDEX IF NOT EXISTS idx_findings_severity ON findings(severity);
CREATE INDEX IF NOT EXISTS idx_findings_status ON findings(status);

CREATE VIRTUAL TABLE IF NOT EXISTS findings_fts USING fts5(
    title,
    description,
    content = 'findings',
    content_rowid = 'rowid'
);

CREATE TRIGGER IF NOT EXISTS findings_ai AFTER INSERT ON findings BEGIN
    INSERT INTO findings_fts(rowid, title, description)
    VALUES (new.rowid, new.title, COALESCE(new.description, ''));
END;

CREATE TRIGGER IF NOT EXISTS findings_ad AFTER DELETE ON findings BEGIN
    INSERT INTO findings_fts(findings_fts, rowid, title, description)
    VALUES ('delete', old.rowid, old.title, COALESCE(old.description, ''));
END;

CREATE TRIGGER IF NOT EXISTS findings_au AFTER UPDATE ON findings BEGIN
    INSERT INTO findings_fts(findings_fts, rowid, title, description)
    VALUES ('delete', old.rowid, old.title, COALESCE(old.description, ''));
    INSERT INTO findings_fts(rowid, title, description)
    VALUES (new.rowid, new.title, COALESCE(new.description, ''));
END;

CREATE TABLE IF NOT EXISTS payloads (
    id            TEXT PRIMARY KEY NOT NULL,
    project_id    TEXT REFERENCES projects(id) ON DELETE SET NULL,
    name          TEXT NOT NULL,
    payload_type  TEXT NOT NULL,
    content       TEXT NOT NULL,
    metadata_json TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_payloads_project_id ON payloads(project_id);

CREATE TABLE IF NOT EXISTS attack_results (
    id             TEXT PRIMARY KEY NOT NULL,
    scan_id        TEXT NOT NULL REFERENCES scans(id) ON DELETE CASCADE,
    payload_id     TEXT REFERENCES payloads(id) ON DELETE SET NULL,
    target_id      TEXT REFERENCES targets(id) ON DELETE SET NULL,
    probe_id       TEXT,
    success        INTEGER NOT NULL DEFAULT 0,
    response_json  TEXT,
    evaluated_json TEXT,
    duration_ms    INTEGER,
    created_at     TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_attack_results_scan_id ON attack_results(scan_id);
CREATE INDEX IF NOT EXISTS idx_attack_results_payload_id ON attack_results(payload_id);

CREATE TABLE IF NOT EXISTS reports (
    id            TEXT PRIMARY KEY NOT NULL,
    project_id    TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    scan_id       TEXT REFERENCES scans(id) ON DELETE SET NULL,
    name          TEXT NOT NULL,
    format        TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'pending',
    file_path     TEXT,
    metadata_json TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_reports_project_id ON reports(project_id);
CREATE INDEX IF NOT EXISTS idx_reports_scan_id ON reports(scan_id);

CREATE TABLE IF NOT EXISTS models (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL,
    file_path       TEXT NOT NULL,
    format          TEXT NOT NULL DEFAULT 'gguf',
    checksum_sha256 TEXT,
    size_bytes      INTEGER,
    metadata_json   TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_models_name ON models(name);

CREATE TABLE IF NOT EXISTS plugins (
    id            TEXT PRIMARY KEY NOT NULL,
    plugin_id     TEXT NOT NULL UNIQUE,
    name          TEXT NOT NULL,
    version       TEXT NOT NULL,
    enabled       INTEGER NOT NULL DEFAULT 0,
    manifest_json TEXT NOT NULL,
    install_path  TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_plugins_enabled ON plugins(enabled);
