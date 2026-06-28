-- AI Endpoint Metadata: replace fingerprint-only storage with full metadata + filter columns.
ALTER TABLE endpoints RENAME COLUMN fingerprint_json TO metadata_json;

ALTER TABLE endpoints ADD COLUMN endpoint_type TEXT NOT NULL DEFAULT 'unknown_ai';
ALTER TABLE endpoints ADD COLUMN ai_framework TEXT;
ALTER TABLE endpoints ADD COLUMN risk_score INTEGER NOT NULL DEFAULT 0;
ALTER TABLE endpoints ADD COLUMN metadata_confidence REAL NOT NULL DEFAULT 0;
ALTER TABLE endpoints ADD COLUMN discovery_source TEXT NOT NULL DEFAULT 'discovery';
ALTER TABLE endpoints ADD COLUMN auth_required INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_endpoints_endpoint_type ON endpoints(endpoint_type);
CREATE INDEX IF NOT EXISTS idx_endpoints_risk_score ON endpoints(risk_score);
CREATE INDEX IF NOT EXISTS idx_endpoints_ai_framework ON endpoints(ai_framework);
