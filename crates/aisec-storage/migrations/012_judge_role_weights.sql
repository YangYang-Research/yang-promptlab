-- Configurable LLM judge role weights for consensus scoring.

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
