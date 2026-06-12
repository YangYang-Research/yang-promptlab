-- Discovered endpoints produced by the discovery engine, scoped to a scan.
CREATE TABLE IF NOT EXISTS endpoints (
    id TEXT PRIMARY KEY NOT NULL,
    scan_id TEXT NOT NULL REFERENCES scans(id) ON DELETE CASCADE,
    target_id TEXT REFERENCES targets(id) ON DELETE SET NULL,
    url TEXT NOT NULL,
    kind TEXT NOT NULL,
    method TEXT,
    confidence REAL NOT NULL DEFAULT 0,
    evidence TEXT,
    source_url TEXT,
    discovered_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_endpoints_scan_id ON endpoints(scan_id);
CREATE INDEX IF NOT EXISTS idx_endpoints_target_id ON endpoints(target_id);
