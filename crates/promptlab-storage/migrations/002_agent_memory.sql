-- Agent short-term memory: session-scoped working memory (chat / scan / ReAct scratchpad).
-- Entries are ephemeral; prune by session_id or expires_at.
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
