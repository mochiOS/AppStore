CREATE TABLE IF NOT EXISTS apps (
    bundle_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    developer TEXT NOT NULL,
    description TEXT NOT NULL,
    icon TEXT
);

INSERT OR IGNORE INTO apps (
    bundle_id,
    name,
    version,
    developer,
    description,
    icon
) VALUES (
    'com.example',
    'ExampleApplication',
    '0.1.0',
    'exampleDeveloper',
    'A example application',
    'assets/icon.png'
);