-- Persist project health score (0–100) computed after a completed attack scan.
-- NULL means not yet scored (no targets / never scanned).

ALTER TABLE projects ADD COLUMN health_score INTEGER;
