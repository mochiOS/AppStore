CREATE TABLE package_suspensions (
  bundle_id TEXT PRIMARY KEY REFERENCES bundle_ids(bundle_id),
  suspended_by_account_id TEXT NOT NULL,
  reason TEXT NOT NULL,
  suspended_at INTEGER NOT NULL
);
