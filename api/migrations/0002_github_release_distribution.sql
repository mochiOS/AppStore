ALTER TABLE releases RENAME TO releases_r2_legacy;

CREATE TABLE releases (
  release_id TEXT PRIMARY KEY,
  bundle_id TEXT NOT NULL REFERENCES apps(bundle_id) ON DELETE CASCADE,
  version TEXT NOT NULL,
  github_repository_id INTEGER,
  github_repository TEXT,
  github_release_id INTEGER,
  github_release_tag TEXT,
  github_release_immutable INTEGER NOT NULL DEFAULT 0 CHECK (github_release_immutable IN (0, 1)),
  github_prerelease INTEGER NOT NULL DEFAULT 0 CHECK (github_prerelease IN (0, 1)),
  github_asset_id INTEGER,
  asset_name TEXT,
  download_url TEXT CHECK (download_url IS NULL OR download_url LIKE 'https://github.com/%/releases/download/%'),
  file_size INTEGER NOT NULL,
  github_digest TEXT,
  github_asset_created_at TEXT,
  github_asset_updated_at TEXT,
  sha256 TEXT,
  manifest_hash TEXT,
  signature TEXT,
  developer_certificate_id TEXT NOT NULL,
  developer_public_key TEXT NOT NULL DEFAULT '',
  minimum_mochios_version TEXT NOT NULL DEFAULT '0',
  changelog TEXT,
  validation_status TEXT NOT NULL DEFAULT 'pending'
    CHECK (validation_status IN ('pending', 'valid', 'invalid')),
  review_status TEXT NOT NULL DEFAULT 'pending'
    CHECK (review_status IN ('pending', 'submitted', 'approved', 'rejected')),
  publish_status TEXT NOT NULL DEFAULT 'draft'
    CHECK (publish_status IN ('draft', 'published', 'revoked')),
  validation_message TEXT,
  review_message TEXT,
  registered_by TEXT,
  created_at INTEGER NOT NULL,
  validated_at INTEGER,
  validated_by TEXT,
  submitted_at INTEGER,
  reviewed_at INTEGER,
  reviewed_by TEXT,
  published_at INTEGER,
  UNIQUE (bundle_id, version)
);

INSERT INTO releases (
  release_id, bundle_id, version, file_size, sha256, manifest_hash, signature,
  developer_certificate_id, changelog, validation_status, review_status,
  publish_status, validation_message, review_message, created_at, reviewed_at,
  reviewed_by, published_at
)
SELECT
  release_id, bundle_id, version, package_size, package_sha256, manifest_hash,
  signature, certificate_id, changelog, 'invalid', 'rejected', 'revoked',
  'R2 legacy release must be resubmitted from GitHub Releases', review_message,
  created_at, reviewed_at, reviewed_by, published_at
FROM releases_r2_legacy;

DROP TABLE releases_r2_legacy;

UPDATE apps SET visibility = 'private', latest_version = NULL;

CREATE INDEX idx_releases_bundle_publish
  ON releases(bundle_id, publish_status, published_at DESC);
CREATE INDEX idx_releases_review
  ON releases(review_status, validation_status, submitted_at DESC);
CREATE UNIQUE INDEX idx_releases_github_asset
  ON releases(github_asset_id)
  WHERE github_asset_id IS NOT NULL;
