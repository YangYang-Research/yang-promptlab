-- AI Target Profile column on targets (Scan Wizard SSOT)

ALTER TABLE targets ADD COLUMN profile_json TEXT NOT NULL DEFAULT '{}';
