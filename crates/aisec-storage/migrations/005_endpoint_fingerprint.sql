-- AI stack fingerprint results attached to discovered endpoints.
ALTER TABLE endpoints ADD COLUMN fingerprint_json TEXT;
