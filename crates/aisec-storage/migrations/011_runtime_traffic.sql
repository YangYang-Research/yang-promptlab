-- Runtime traffic monitor events (sent/received packages) for historical charts.

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
