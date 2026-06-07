CREATE TABLE IF NOT EXISTS developers (
    developer_id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('active', 'suspended', 'revoked'))
);

CREATE TABLE IF NOT EXISTS oauth_links (
    developer_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    provider_subject_hash TEXT NOT NULL,
    linked_at TEXT NOT NULL,
    provider_username TEXT,
    updated_at TEXT,
    PRIMARY KEY (provider, provider_subject_hash),
    FOREIGN KEY (developer_id) REFERENCES developers(developer_id)
);

CREATE TABLE IF NOT EXISTS public_keys (
    key_id TEXT PRIMARY KEY,
    developer_id TEXT NOT NULL,
    public_key TEXT NOT NULL,
    fingerprint TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    revoked_at TEXT,
    FOREIGN KEY (developer_id) REFERENCES developers(developer_id)
);

CREATE TABLE IF NOT EXISTS bundle_ids (
    bundle_id TEXT PRIMARY KEY,
    developer_id TEXT NOT NULL,
    app_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('reserved', 'active', 'blocked')),
    created_at TEXT NOT NULL,
    FOREIGN KEY (developer_id) REFERENCES developers(developer_id)
);

CREATE TABLE IF NOT EXISTS developer_apps (
    app_id TEXT PRIMARY KEY,
    bundle_id TEXT NOT NULL UNIQUE,
    latest_version TEXT,
    display_name TEXT NOT NULL,
    icon_path TEXT,
    description TEXT,
    visibility TEXT NOT NULL CHECK(visibility IN ('private', 'public')),
    created_at TEXT NOT NULL,
    FOREIGN KEY (bundle_id) REFERENCES bundle_ids(bundle_id)
);

CREATE TABLE IF NOT EXISTS developer_releases (
    release_id TEXT PRIMARY KEY,
    bundle_id TEXT NOT NULL,
    version TEXT NOT NULL,

    manifest_hash TEXT,
    package_hash TEXT NOT NULL,
    signature TEXT,
    certificate_id TEXT,

    status TEXT NOT NULL CHECK(status IN (
    'draft',
    'submitted',
    'approved',
    'rejected',
    'published',
    'revoked'
)),
    created_at TEXT NOT NULL,
    package_path TEXT,
    package_size INTEGER,
    changelog TEXT,
    review_message TEXT,
    submitted_at TEXT,
    reviewed_at TEXT,
    reviewed_by TEXT,
    published_at TEXT,
    UNIQUE (bundle_id, version),
    FOREIGN KEY (bundle_id) REFERENCES bundle_ids(bundle_id)
);

CREATE TABLE IF NOT EXISTS revocations (
    revocation_id TEXT PRIMARY KEY,
    target_type TEXT NOT NULL CHECK(target_type IN ('developer', 'key', 'certificate', 'app', 'release')),
    target_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    created_at TEXT NOT NULL
);
