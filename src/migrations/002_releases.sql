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

INSERT OR IGNORE INTO releases (
    bundle_id,
    version,
    size,
    sha256,
    changelog,
    download_path,
    created_at
) VALUES (
    'com.example',
    '0.1.0',
    50,
    '11795b398f94aa489c1e45613bf37c295994eb34ede6553e28f1f85df1ceaf4e',
    'Initial release',
    'data/releases/com.example/0.1.0.pkg',
    '2026-06-04T00:00:00+09:00'
);
