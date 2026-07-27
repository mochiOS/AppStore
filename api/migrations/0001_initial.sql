PRAGMA foreign_keys = ON;

CREATE TABLE bundle_ids (
  bundle_id TEXT PRIMARY KEY,
  developer_id TEXT NOT NULL,
  app_name TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('reserved', 'active', 'blocked')),
  created_at INTEGER NOT NULL
);

CREATE TABLE apps (
  app_id TEXT PRIMARY KEY,
  bundle_id TEXT NOT NULL UNIQUE REFERENCES bundle_ids(bundle_id),
  developer_id TEXT NOT NULL,
  display_name TEXT NOT NULL,
  subtitle TEXT,
  description TEXT NOT NULL DEFAULT '',
  icon_url TEXT,
  category TEXT,
  kind TEXT NOT NULL DEFAULT 'app' CHECK (kind IN ('app', 'game')),
  price_label TEXT NOT NULL DEFAULT '入手',
  age_rating TEXT,
  visibility TEXT NOT NULL DEFAULT 'private' CHECK (visibility IN ('private', 'public')),
  latest_version TEXT,
  team_id TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE releases (
  release_id TEXT PRIMARY KEY,
  bundle_id TEXT NOT NULL REFERENCES apps(bundle_id) ON DELETE CASCADE,
  version TEXT NOT NULL,
  package_key TEXT NOT NULL UNIQUE,
  package_size INTEGER NOT NULL,
  package_sha256 TEXT NOT NULL,
  manifest_hash TEXT,
  signature TEXT NOT NULL,
  certificate_id TEXT NOT NULL,
  changelog TEXT,
  status TEXT NOT NULL CHECK (status IN ('draft', 'submitted', 'published', 'rejected', 'revoked')),
  review_message TEXT,
  created_at INTEGER NOT NULL,
  submitted_at INTEGER,
  reviewed_at INTEGER,
  reviewed_by TEXT,
  published_at INTEGER,
  UNIQUE (bundle_id, version)
);

CREATE TABLE app_screenshots (
  bundle_id TEXT NOT NULL REFERENCES apps(bundle_id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  image_url TEXT NOT NULL,
  PRIMARY KEY (bundle_id, position)
);

CREATE TABLE ratings (
  bundle_id TEXT PRIMARY KEY REFERENCES apps(bundle_id) ON DELETE CASCADE,
  rating_sum INTEGER NOT NULL DEFAULT 0,
  rating_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE public_keys (
  key_id TEXT PRIMARY KEY,
  developer_id TEXT NOT NULL,
  public_key TEXT NOT NULL,
  fingerprint TEXT NOT NULL UNIQUE,
  created_at INTEGER NOT NULL,
  revoked_at INTEGER
);

CREATE TABLE teams (
  team_id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE team_members (
  team_id TEXT NOT NULL REFERENCES teams(team_id) ON DELETE CASCADE,
  developer_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'developer', 'viewer')),
  joined_at INTEGER NOT NULL,
  PRIMARY KEY (team_id, developer_id)
);

CREATE TABLE audit_logs (
  audit_id TEXT PRIMARY KEY,
  actor_id TEXT,
  action TEXT NOT NULL,
  target_type TEXT NOT NULL,
  target_id TEXT NOT NULL,
  metadata_json TEXT,
  created_at INTEGER NOT NULL
);

CREATE INDEX idx_apps_public ON apps(visibility, kind, category, updated_at DESC);
CREATE INDEX idx_releases_bundle_status ON releases(bundle_id, status, published_at DESC);
CREATE INDEX idx_releases_review ON releases(status, submitted_at DESC);
CREATE INDEX idx_bundle_ids_developer ON bundle_ids(developer_id, created_at DESC);
CREATE INDEX idx_apps_developer ON apps(developer_id, created_at DESC);
CREATE INDEX idx_team_members_developer ON team_members(developer_id);
CREATE INDEX idx_audit_target ON audit_logs(target_type, target_id, created_at DESC);
