CREATE TABLE IF NOT EXISTS apps (
    bundle_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    developer TEXT NOT NULL,
    description TEXT NOT NULL,
    icon TEXT
);