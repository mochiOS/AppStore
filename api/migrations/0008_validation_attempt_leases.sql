ALTER TABLE releases ADD COLUMN validation_attempt_id TEXT;
ALTER TABLE releases ADD COLUMN validation_started_at INTEGER;

CREATE UNIQUE INDEX idx_releases_validation_attempt
  ON releases(validation_attempt_id)
  WHERE validation_attempt_id IS NOT NULL;
