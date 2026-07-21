-- Global attack technique catalog (editable prompts). Replaces embedded payloads.json at runtime.

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
