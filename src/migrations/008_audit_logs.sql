CREATE TABLE IF NOT EXISTS audit_logs (
    audit_id TEXT PRIMARY KEY,
    actor_developer_id TEXT,
    action TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    metadata_json TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_target
    ON audit_logs(target_type, target_id);

CREATE INDEX IF NOT EXISTS idx_audit_logs_actor
    ON audit_logs(actor_developer_id);
