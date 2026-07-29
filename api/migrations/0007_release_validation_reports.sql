ALTER TABLE releases ADD COLUMN registered_by_account_id TEXT;
ALTER TABLE releases ADD COLUMN developer_display_name TEXT NOT NULL DEFAULT '';
ALTER TABLE releases ADD COLUMN package_digest TEXT;
ALTER TABLE releases ADD COLUMN capabilities_json TEXT;
ALTER TABLE releases ADD COLUMN payloads_json TEXT;
ALTER TABLE releases ADD COLUMN reviewer_version TEXT;
ALTER TABLE releases ADD COLUMN validation_error_code TEXT;
ALTER TABLE releases ADD COLUMN rejection_reason_code TEXT;
ALTER TABLE releases ADD COLUMN rejection_note TEXT;
ALTER TABLE releases ADD COLUMN withdrawn_at INTEGER;
ALTER TABLE releases ADD COLUMN last_integrity_checked_at INTEGER;

CREATE TRIGGER audit_logs_append_only_update
BEFORE UPDATE ON audit_logs
BEGIN
  SELECT RAISE(ABORT, 'audit logs are append-only');
END;

CREATE TRIGGER audit_logs_append_only_delete
BEFORE DELETE ON audit_logs
BEGIN
  SELECT RAISE(ABORT, 'audit logs are append-only');
END;
