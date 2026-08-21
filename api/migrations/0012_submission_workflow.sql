CREATE TABLE app_certificates (
  app_id TEXT PRIMARY KEY REFERENCES apps(app_id) ON DELETE CASCADE,
  certificate_id TEXT NOT NULL UNIQUE,
  assigned_by_account_id TEXT NOT NULL,
  assigned_at INTEGER NOT NULL,
  last_verified_at INTEGER,
  observed_status TEXT NOT NULL DEFAULT 'active'
    CHECK (observed_status IN ('active', 'suspended', 'revoked')),
  UNIQUE (app_id, certificate_id)
);

CREATE TABLE app_builds (
  build_id TEXT PRIMARY KEY,
  app_id TEXT NOT NULL REFERENCES apps(app_id) ON DELETE CASCADE,
  certificate_id TEXT NOT NULL,
  version TEXT NOT NULL,
  build_number INTEGER NOT NULL CHECK (build_number > 0),
  github_repository_id INTEGER NOT NULL,
  github_repository TEXT NOT NULL,
  github_release_id INTEGER NOT NULL,
  github_release_tag TEXT NOT NULL,
  github_asset_id INTEGER NOT NULL UNIQUE,
  asset_name TEXT NOT NULL,
  download_url TEXT NOT NULL
    CHECK (download_url LIKE 'https://github.com/%/releases/download/%'),
  file_size INTEGER NOT NULL CHECK (file_size > 0),
  sha256 TEXT,
  package_digest TEXT,
  manifest_digest TEXT,
  capabilities_json TEXT,
  payloads_json TEXT,
  machine_status TEXT NOT NULL DEFAULT 'pending'
    CHECK (machine_status IN ('pending', 'valid', 'invalid')),
  machine_message TEXT,
  reviewer_version TEXT,
  registered_by_account_id TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  validated_at INTEGER,
  FOREIGN KEY (app_id, certificate_id)
    REFERENCES app_certificates(app_id, certificate_id),
  UNIQUE (app_id, version, build_number)
);

CREATE TABLE submissions (
  submission_id TEXT PRIMARY KEY,
  app_id TEXT NOT NULL REFERENCES apps(app_id) ON DELETE CASCADE,
  build_id TEXT NOT NULL REFERENCES app_builds(build_id),
  version TEXT NOT NULL,
  submission_number INTEGER NOT NULL CHECK (submission_number > 0),
  submission_kind TEXT NOT NULL
    CHECK (submission_kind IN ('new_app', 'update', 're_review')),
  state TEXT NOT NULL DEFAULT 'draft'
    CHECK (state IN (
      'draft', 'submitted', 'in_review', 'approved', 'changes_required',
      'more_information_required', 'rejected'
    )),
  previous_submission_id TEXT REFERENCES submissions(submission_id),
  created_by_account_id TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  submitted_at INTEGER,
  resolved_at INTEGER,
  UNIQUE (app_id, submission_number)
);

CREATE TABLE submission_details (
  submission_id TEXT PRIMARY KEY REFERENCES submissions(submission_id) ON DELETE CASCADE,
  app_name TEXT NOT NULL,
  developer_name TEXT NOT NULL,
  description TEXT NOT NULL,
  icon_url TEXT NOT NULL,
  icon_media_type TEXT NOT NULL CHECK (icon_media_type IN ('image/png', 'image/jpeg')),
  icon_width INTEGER NOT NULL CHECK (icon_width = 512),
  icon_height INTEGER NOT NULL CHECK (icon_height = 512),
  category TEXT,
  kind TEXT NOT NULL CHECK (kind IN ('app', 'game')),
  release_channel TEXT NOT NULL DEFAULT 'stable'
    CHECK (release_channel IN ('stable', 'alpha', 'beta', 'experimental')),
  primary_purpose TEXT NOT NULL DEFAULT 'general'
    CHECK (primary_purpose IN ('general', 'medical', 'financial')),
  age_rating TEXT,
  external_communication INTEGER NOT NULL CHECK (external_communication IN (0, 1)),
  external_communication_reason TEXT,
  external_communication_purpose TEXT,
  collects_data INTEGER NOT NULL CHECK (collects_data IN (0, 1)),
  data_collection_description TEXT,
  uses_advertising INTEGER NOT NULL CHECK (uses_advertising IN (0, 1)),
  uses_analytics INTEGER NOT NULL CHECK (uses_analytics IN (0, 1)),
  tracks_across_services INTEGER NOT NULL CHECK (tracks_across_services IN (0, 1)),
  tracking_user_consent INTEGER NOT NULL CHECK (tracking_user_consent IN (0, 1)),
  uses_location_for_advertising INTEGER NOT NULL CHECK (uses_location_for_advertising IN (0, 1)),
  has_payments INTEGER NOT NULL CHECK (has_payments IN (0, 1)),
  content_declarations_json TEXT NOT NULL DEFAULT '{}',
  executes_dynamic_code INTEGER NOT NULL CHECK (executes_dynamic_code IN (0, 1)),
  dynamic_code_explanation TEXT,
  uses_external_updates INTEGER NOT NULL CHECK (uses_external_updates IN (0, 1)),
  external_updates_explanation TEXT,
  is_emulator INTEGER NOT NULL CHECK (is_emulator IN (0, 1)),
  is_virtual_machine INTEGER NOT NULL CHECK (is_virtual_machine IN (0, 1)),
  supports_plugins INTEGER NOT NULL CHECK (supports_plugins IN (0, 1)),
  is_external_app_store INTEGER NOT NULL CHECK (is_external_app_store IN (0, 1)),
  uses_ai_generated_content INTEGER NOT NULL CHECK (uses_ai_generated_content IN (0, 1)),
  disclose_ai_generated_content INTEGER NOT NULL CHECK (disclose_ai_generated_content IN (0, 1)),
  reviewer_notes TEXT,
  requires_login INTEGER NOT NULL CHECK (requires_login IN (0, 1)),
  test_account TEXT,
  test_instructions TEXT
);

CREATE TABLE submission_screenshots (
  submission_id TEXT NOT NULL REFERENCES submissions(submission_id) ON DELETE CASCADE,
  position INTEGER NOT NULL CHECK (position >= 0),
  image_url TEXT NOT NULL,
  contains_actual_app_ui INTEGER NOT NULL CHECK (contains_actual_app_ui IN (0, 1)),
  PRIMARY KEY (submission_id, position)
);

CREATE TABLE submission_capabilities (
  submission_id TEXT NOT NULL REFERENCES submissions(submission_id) ON DELETE CASCADE,
  capability TEXT NOT NULL,
  source TEXT NOT NULL DEFAULT 'manifest' CHECK (source = 'manifest'),
  usage_reason TEXT,
  PRIMARY KEY (submission_id, capability)
);

CREATE TABLE submission_network_domains (
  submission_id TEXT NOT NULL REFERENCES submissions(submission_id) ON DELETE CASCADE,
  domain TEXT NOT NULL CHECK (
    length(trim(domain)) > 0
    AND instr(domain, '*') = 0
    AND instr(domain, '://') = 0
    AND instr(domain, '/') = 0
  ),
  PRIMARY KEY (submission_id, domain)
);

CREATE TABLE submission_data_categories (
  submission_id TEXT NOT NULL REFERENCES submissions(submission_id) ON DELETE CASCADE,
  category TEXT NOT NULL,
  details TEXT,
  PRIMARY KEY (submission_id, category)
);

CREATE TABLE submission_reviews (
  review_id TEXT PRIMARY KEY,
  submission_id TEXT NOT NULL REFERENCES submissions(submission_id),
  reviewer_account_id TEXT NOT NULL,
  decision TEXT NOT NULL CHECK (decision IN (
    'approved', 'changes_required', 'more_information_required', 'rejected'
  )),
  reason TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE submission_messages (
  message_id TEXT PRIMARY KEY,
  submission_id TEXT NOT NULL REFERENCES submissions(submission_id),
  author_account_id TEXT NOT NULL,
  author_role TEXT NOT NULL CHECK (author_role IN ('developer', 'reviewer')),
  body TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE appeals (
  appeal_id TEXT PRIMARY KEY,
  app_id TEXT NOT NULL REFERENCES apps(app_id) ON DELETE CASCADE,
  submission_id TEXT REFERENCES submissions(submission_id),
  appealed_action TEXT NOT NULL CHECK (appealed_action IN ('review_decision', 'removed')),
  reason TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'submitted'
    CHECK (state IN ('submitted', 'in_review', 'resolved')),
  resolution TEXT,
  submitted_by_account_id TEXT NOT NULL,
  resolved_by_account_id TEXT,
  created_at INTEGER NOT NULL,
  resolved_at INTEGER
);

CREATE TABLE app_availability (
  app_id TEXT PRIMARY KEY REFERENCES apps(app_id) ON DELETE CASCADE,
  status TEXT NOT NULL DEFAULT 'not_available'
    CHECK (status IN ('not_available', 'available', 'developer_unpublished', 'removed')),
  current_submission_id TEXT REFERENCES submissions(submission_id),
  reason TEXT,
  changed_by_account_id TEXT,
  changed_at INTEGER NOT NULL
);

CREATE TABLE published_versions (
  app_id TEXT NOT NULL REFERENCES apps(app_id) ON DELETE CASCADE,
  version TEXT NOT NULL,
  submission_id TEXT NOT NULL UNIQUE REFERENCES submissions(submission_id),
  published_at INTEGER NOT NULL,
  PRIMARY KEY (app_id, version)
);

CREATE TABLE app_acquisitions (
  app_id TEXT NOT NULL REFERENCES apps(app_id) ON DELETE CASCADE,
  account_id TEXT NOT NULL,
  first_acquired_at INTEGER NOT NULL,
  PRIMARY KEY (app_id, account_id)
);

CREATE TABLE availability_history (
  event_id TEXT PRIMARY KEY,
  app_id TEXT NOT NULL REFERENCES apps(app_id),
  from_status TEXT,
  to_status TEXT NOT NULL,
  reason TEXT,
  actor_account_id TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE INDEX idx_app_builds_app_created
  ON app_builds(app_id, created_at DESC);
CREATE INDEX idx_submissions_app_created
  ON submissions(app_id, created_at DESC);
CREATE INDEX idx_submissions_queue
  ON submissions(state, submitted_at, created_at);
CREATE INDEX idx_reviews_submission_created
  ON submission_reviews(submission_id, created_at DESC);
CREATE INDEX idx_messages_submission_created
  ON submission_messages(submission_id, created_at);
CREATE INDEX idx_appeals_queue
  ON appeals(state, created_at);
CREATE INDEX idx_availability_status
  ON app_availability(status, changed_at DESC);
CREATE INDEX idx_availability_history_app
  ON availability_history(app_id, created_at DESC);

CREATE TRIGGER submission_reviews_no_update BEFORE UPDATE ON submission_reviews
BEGIN SELECT RAISE(ABORT, 'submission reviews are append-only'); END;

CREATE TRIGGER submission_reviews_no_delete BEFORE DELETE ON submission_reviews
BEGIN SELECT RAISE(ABORT, 'submission reviews are append-only'); END;

CREATE TRIGGER availability_history_no_update BEFORE UPDATE ON availability_history
BEGIN SELECT RAISE(ABORT, 'availability history is append-only'); END;

CREATE TRIGGER availability_history_no_delete BEFORE DELETE ON availability_history
BEGIN SELECT RAISE(ABORT, 'availability history is append-only'); END;

CREATE TRIGGER app_acquisitions_no_update BEFORE UPDATE ON app_acquisitions
BEGIN SELECT RAISE(ABORT, 'app acquisitions are append-only'); END;

CREATE TRIGGER app_acquisitions_no_delete BEFORE DELETE ON app_acquisitions
BEGIN SELECT RAISE(ABORT, 'app acquisitions are append-only'); END;
