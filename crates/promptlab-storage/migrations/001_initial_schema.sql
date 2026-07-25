-- PromptLab consolidated schema (fresh installs).
-- Includes core tables, agent memory, and mutator settings.

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS projects (
    id           TEXT PRIMARY KEY NOT NULL,
    name         TEXT NOT NULL,
    description  TEXT,
    summary_json TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS targets (
    id              TEXT PRIMARY KEY NOT NULL,
    project_id      TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    target_type     TEXT NOT NULL,
    descriptor_json TEXT NOT NULL DEFAULT '{}',
    profile_json    TEXT NOT NULL DEFAULT '{}',
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_targets_project_id ON targets(project_id);

CREATE TABLE IF NOT EXISTS scans (
    id            TEXT PRIMARY KEY NOT NULL,
    project_id    TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    target_id     TEXT REFERENCES targets(id) ON DELETE CASCADE,
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

CREATE TABLE IF NOT EXISTS auth_profiles (
    id                      TEXT PRIMARY KEY NOT NULL,
    project_id              TEXT REFERENCES projects(id) ON DELETE CASCADE,
    name                    TEXT NOT NULL,
    method                  TEXT NOT NULL,
    config_json             TEXT NOT NULL,
    credential_reference_id TEXT,
    created_at              TEXT NOT NULL,
    updated_at              TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_auth_profiles_project_id ON auth_profiles(project_id);
CREATE INDEX IF NOT EXISTS idx_auth_profiles_method ON auth_profiles(method);
CREATE INDEX IF NOT EXISTS idx_auth_profiles_credential_ref
    ON auth_profiles(credential_reference_id);

CREATE TABLE IF NOT EXISTS auth_sessions (
    id                      TEXT PRIMARY KEY NOT NULL,
    profile_id              TEXT NOT NULL REFERENCES auth_profiles(id) ON DELETE CASCADE,
    status                  TEXT NOT NULL DEFAULT 'active',
    cookies_json            TEXT,
    tokens_json             TEXT,
    storage_state_path      TEXT,
    expires_at              TEXT,
    validation_status       TEXT NOT NULL DEFAULT 'valid',
    last_validated_at       TEXT,
    user_identity           TEXT,
    credential_reference_id TEXT,
    created_at              TEXT NOT NULL,
    updated_at              TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_auth_sessions_profile_id ON auth_sessions(profile_id);
CREATE INDEX IF NOT EXISTS idx_auth_sessions_status ON auth_sessions(status);
CREATE INDEX IF NOT EXISTS idx_auth_sessions_credential_ref
    ON auth_sessions(credential_reference_id);

CREATE TABLE IF NOT EXISTS auth_recordings (
    id                 TEXT PRIMARY KEY NOT NULL,
    profile_id         TEXT NOT NULL REFERENCES auth_profiles(id) ON DELETE CASCADE,
    steps_json         TEXT NOT NULL,
    storage_state_path TEXT,
    metadata_json      TEXT,
    created_at         TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_auth_recordings_profile_id ON auth_recordings(profile_id);

CREATE TABLE IF NOT EXISTS endpoints (
    id                   TEXT PRIMARY KEY NOT NULL,
    scan_id              TEXT NOT NULL REFERENCES scans(id) ON DELETE CASCADE,
    target_id            TEXT REFERENCES targets(id) ON DELETE SET NULL,
    url                  TEXT NOT NULL,
    kind                 TEXT NOT NULL,
    method               TEXT,
    confidence           REAL NOT NULL DEFAULT 0,
    evidence             TEXT,
    source_url           TEXT,
    discovered_at        TEXT NOT NULL,
    created_at           TEXT NOT NULL,
    metadata_json        TEXT,
    endpoint_type        TEXT NOT NULL DEFAULT 'unknown_ai',
    ai_framework         TEXT,
    risk_score           INTEGER NOT NULL DEFAULT 0,
    metadata_confidence  REAL NOT NULL DEFAULT 0,
    discovery_source     TEXT NOT NULL DEFAULT 'discovery',
    auth_required        INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_endpoints_scan_id ON endpoints(scan_id);
CREATE INDEX IF NOT EXISTS idx_endpoints_target_id ON endpoints(target_id);
CREATE INDEX IF NOT EXISTS idx_endpoints_endpoint_type ON endpoints(endpoint_type);
CREATE INDEX IF NOT EXISTS idx_endpoints_risk_score ON endpoints(risk_score);
CREATE INDEX IF NOT EXISTS idx_endpoints_ai_framework ON endpoints(ai_framework);

CREATE TABLE IF NOT EXISTS attack_catalog_techniques (
    id              TEXT PRIMARY KEY NOT NULL,
    category_id     TEXT NOT NULL,
    name            TEXT NOT NULL,
    description     TEXT,
    content         TEXT NOT NULL,
    default_content TEXT NOT NULL,
    tags_json       TEXT NOT NULL DEFAULT '[]',
    surface         TEXT,
    owasp           TEXT,
    enabled         INTEGER NOT NULL DEFAULT 1,
    user_modified   INTEGER NOT NULL DEFAULT 0,
    sort_order      INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_attack_catalog_category
    ON attack_catalog_techniques(category_id);

CREATE INDEX IF NOT EXISTS idx_attack_catalog_enabled
    ON attack_catalog_techniques(enabled);

CREATE TABLE IF NOT EXISTS runtime_traffic_events (
    id          TEXT PRIMARY KEY NOT NULL,
    at_ms       INTEGER NOT NULL,
    direction   TEXT NOT NULL CHECK (direction IN ('sent', 'received')),
    created_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_runtime_traffic_at_ms
    ON runtime_traffic_events(at_ms);

CREATE INDEX IF NOT EXISTS idx_runtime_traffic_direction_at_ms
    ON runtime_traffic_events(direction, at_ms);

CREATE TABLE IF NOT EXISTS runtime_traffic_counters (
    id                INTEGER PRIMARY KEY CHECK (id = 1),
    lifetime_sent     INTEGER NOT NULL DEFAULT 0,
    lifetime_received INTEGER NOT NULL DEFAULT 0
);

INSERT OR IGNORE INTO runtime_traffic_counters (id, lifetime_sent, lifetime_received)
VALUES (1, 0, 0);

-- Consensus-biased defaults: judge 0.85 / classifier 0.80 / attacker 0.75 / default_llm 0.65
CREATE TABLE IF NOT EXISTS judge_role_weights (
    id          INTEGER PRIMARY KEY CHECK (id = 1),
    judge       REAL NOT NULL DEFAULT 0.85,
    classifier  REAL NOT NULL DEFAULT 0.80,
    attacker    REAL NOT NULL DEFAULT 0.75,
    default_llm REAL NOT NULL DEFAULT 0.65,
    updated_at  TEXT NOT NULL
);

INSERT OR IGNORE INTO judge_role_weights (id, judge, classifier, attacker, default_llm, updated_at)
VALUES (1, 0.85, 0.80, 0.75, 0.65, '1970-01-01T00:00:00Z');

-- Agent short-term memory: session-scoped working memory (chat / scan / ReAct scratchpad).
CREATE TABLE IF NOT EXISTS agent_short_term_memory (
    id              TEXT PRIMARY KEY NOT NULL,
    session_id      TEXT NOT NULL,
    agent_id        TEXT NOT NULL,
    project_id      TEXT REFERENCES projects(id) ON DELETE CASCADE,
    target_id       TEXT REFERENCES targets(id) ON DELETE SET NULL,
    scan_id         TEXT REFERENCES scans(id) ON DELETE SET NULL,
    role            TEXT NOT NULL,
    memory_key      TEXT,
    content         TEXT NOT NULL,
    content_json    TEXT,
    importance      REAL NOT NULL DEFAULT 0.5,
    expires_at      TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_stm_session_id
    ON agent_short_term_memory(session_id);

CREATE INDEX IF NOT EXISTS idx_agent_stm_agent_session
    ON agent_short_term_memory(agent_id, session_id);

CREATE INDEX IF NOT EXISTS idx_agent_stm_expires_at
    ON agent_short_term_memory(expires_at);

CREATE INDEX IF NOT EXISTS idx_agent_stm_project_id
    ON agent_short_term_memory(project_id);

-- Agent long-term memory: durable facts across sessions (keyed upsert).
CREATE TABLE IF NOT EXISTS agent_long_term_memory (
    id               TEXT PRIMARY KEY NOT NULL,
    agent_id         TEXT NOT NULL,
    scope_type       TEXT NOT NULL CHECK (
        scope_type IN ('global', 'project', 'target', 'scan')
    ),
    scope_id         TEXT NOT NULL DEFAULT '',
    memory_key       TEXT NOT NULL,
    content          TEXT NOT NULL,
    content_json     TEXT,
    importance       REAL NOT NULL DEFAULT 0.5,
    access_count     INTEGER NOT NULL DEFAULT 0,
    last_accessed_at TEXT,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    UNIQUE (agent_id, scope_type, scope_id, memory_key)
);

CREATE INDEX IF NOT EXISTS idx_agent_ltm_scope
    ON agent_long_term_memory(scope_type, scope_id);

CREATE INDEX IF NOT EXISTS idx_agent_ltm_agent_scope
    ON agent_long_term_memory(agent_id, scope_type, scope_id);

CREATE INDEX IF NOT EXISTS idx_agent_ltm_importance
    ON agent_long_term_memory(importance DESC);

-- App-wide attack mutator settings (Advanced → Mutators).
CREATE TABLE IF NOT EXISTS mutator_settings (
    id                       INTEGER PRIMARY KEY CHECK (id = 1),
    enabled_mutators_json    TEXT NOT NULL,
    category_mutators_json   TEXT NOT NULL,
    updated_at               TEXT NOT NULL
);

INSERT OR IGNORE INTO mutator_settings (
    id,
    enabled_mutators_json,
    category_mutators_json,
    updated_at
)
VALUES (
    1,
    '["base64_wrap","unicode_homoglyph","delimiter_injection","role_swap","chunk_split","json_escape","repeat_amplify","hex_wrap","html_wrap","rot13_wrap","leetspeak","reversed_text","token_split","markdown_code_fence","zero_width_dense"]',
    '{"prompt_injection":["delimiter_injection","role_swap","markdown_code_fence","base64_wrap","html_wrap","hex_wrap","token_split"],"jailbreak":["role_swap","unicode_homoglyph","base64_wrap","html_wrap","leetspeak","chunk_split","zero_width_dense","rot13_wrap","reversed_text"],"system_prompt_extraction":["repeat_amplify","delimiter_injection","role_swap","markdown_code_fence","base64_wrap","hex_wrap"],"tool_abuse":["json_escape","html_wrap","base64_wrap","delimiter_injection","token_split","markdown_code_fence"],"mcp_abuse":["json_escape","html_wrap","base64_wrap","delimiter_injection","token_split","markdown_code_fence"],"rag_leakage":["repeat_amplify","delimiter_injection","markdown_code_fence","zero_width_dense","token_split"],"memory_poisoning":["repeat_amplify","role_swap","delimiter_injection","markdown_code_fence","chunk_split"],"cross_user_leakage":["role_swap","delimiter_injection","repeat_amplify","markdown_code_fence","zero_width_dense"],"agent_goal_hijacking":["role_swap","delimiter_injection","markdown_code_fence","repeat_amplify","token_split"]}',
    '1970-01-01T00:00:00Z'
);
