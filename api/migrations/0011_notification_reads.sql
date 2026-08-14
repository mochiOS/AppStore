CREATE TABLE notification_reads (
  notification_id TEXT NOT NULL REFERENCES audit_logs(audit_id),
  account_id TEXT NOT NULL,
  read_at INTEGER NOT NULL,
  PRIMARY KEY (notification_id, account_id)
);

CREATE INDEX idx_notification_reads_account
  ON notification_reads(account_id, read_at DESC);

CREATE INDEX idx_audit_logs_action_created
  ON audit_logs(action, created_at DESC);
