-- Prompt Lab expand history (seed + variants JSON).
CREATE TABLE IF NOT EXISTS prompt_lab_history (
    id TEXT PRIMARY KEY NOT NULL,
    seed_prompt TEXT NOT NULL,
    variants_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_prompt_lab_history_created_at
    ON prompt_lab_history (created_at DESC);
