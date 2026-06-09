CREATE TABLE IF NOT EXISTS releases (
    bundle_id TEXT NOT NULL,
    version TEXT NOT NULL,
    size INTEGER NOT NULL,
    sha256 TEXT NOT NULL,
    changelog TEXT NOT NULL,
    download_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (bundle_id, version),
    FOREIGN KEY (bundle_id) REFERENCES apps(bundle_id) ON DELETE CASCADE
);
