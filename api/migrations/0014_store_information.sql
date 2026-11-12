CREATE TABLE app_store_information (
  app_id TEXT PRIMARY KEY REFERENCES apps(app_id),
  information_json TEXT NOT NULL CHECK (json_valid(information_json)),
  revision INTEGER NOT NULL CHECK (revision > 0),
  updated_by_account_id TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);
ALTER TABLE submissions ADD COLUMN store_information_revision INTEGER;
ALTER TABLE submissions ADD COLUMN release_notes TEXT NOT NULL DEFAULT '';
ALTER TABLE submissions ADD COLUMN declarations_confirmed INTEGER NOT NULL DEFAULT 0 CHECK (declarations_confirmed IN (0,1));

CREATE TRIGGER submission_snapshot_frozen
BEFORE UPDATE OF submission_id,app_id,build_id,version,submission_number,submission_kind,previous_submission_id,
  created_by_account_id,created_at,
  store_information_revision,release_notes,declarations_confirmed ON submissions
WHEN OLD.state != 'draft'
BEGIN SELECT RAISE(ABORT, 'submitted snapshot is immutable'); END;

CREATE TRIGGER submission_information_current
BEFORE UPDATE OF state ON submissions
WHEN NEW.state='submitted' AND (
  NEW.store_information_revision IS NULL OR NOT EXISTS (
    SELECT 1 FROM app_store_information i
    WHERE i.app_id=NEW.app_id AND i.revision=NEW.store_information_revision
  )
)
BEGIN SELECT RAISE(ABORT, 'store information changed'); END;

CREATE TRIGGER submission_details_insert_draft_only
BEFORE INSERT ON submission_details
WHEN (SELECT state FROM submissions WHERE submission_id=NEW.submission_id) != 'draft'
BEGIN SELECT RAISE(ABORT, 'submitted snapshot is immutable'); END;

CREATE TRIGGER submission_details_update_draft_only
BEFORE UPDATE ON submission_details
WHEN (SELECT state FROM submissions WHERE submission_id=OLD.submission_id) != 'draft' OR (SELECT state FROM submissions WHERE submission_id=NEW.submission_id) != 'draft'
BEGIN SELECT RAISE(ABORT, 'submitted snapshot is immutable'); END;

CREATE TRIGGER submission_details_delete_draft_only
BEFORE DELETE ON submission_details
WHEN (SELECT state FROM submissions WHERE submission_id=OLD.submission_id) != 'draft'
BEGIN SELECT RAISE(ABORT, 'submitted snapshot is immutable'); END;

CREATE TRIGGER submission_screenshots_insert_draft_only
BEFORE INSERT ON submission_screenshots
WHEN (SELECT state FROM submissions WHERE submission_id=NEW.submission_id) != 'draft'
BEGIN SELECT RAISE(ABORT, 'submitted snapshot is immutable'); END;

CREATE TRIGGER submission_screenshots_update_draft_only
BEFORE UPDATE ON submission_screenshots
WHEN (SELECT state FROM submissions WHERE submission_id=OLD.submission_id) != 'draft' OR (SELECT state FROM submissions WHERE submission_id=NEW.submission_id) != 'draft'
BEGIN SELECT RAISE(ABORT, 'submitted snapshot is immutable'); END;

CREATE TRIGGER submission_screenshots_delete_draft_only
BEFORE DELETE ON submission_screenshots
WHEN (SELECT state FROM submissions WHERE submission_id=OLD.submission_id) != 'draft'
BEGIN SELECT RAISE(ABORT, 'submitted snapshot is immutable'); END;

CREATE TRIGGER submission_capabilities_insert_draft_only
BEFORE INSERT ON submission_capabilities
WHEN (SELECT state FROM submissions WHERE submission_id=NEW.submission_id) != 'draft'
BEGIN SELECT RAISE(ABORT, 'submitted snapshot is immutable'); END;

CREATE TRIGGER submission_capabilities_update_draft_only
BEFORE UPDATE ON submission_capabilities
WHEN (SELECT state FROM submissions WHERE submission_id=OLD.submission_id) != 'draft' OR (SELECT state FROM submissions WHERE submission_id=NEW.submission_id) != 'draft'
BEGIN SELECT RAISE(ABORT, 'submitted snapshot is immutable'); END;

CREATE TRIGGER submission_capabilities_delete_draft_only
BEFORE DELETE ON submission_capabilities
WHEN (SELECT state FROM submissions WHERE submission_id=OLD.submission_id) != 'draft'
BEGIN SELECT RAISE(ABORT, 'submitted snapshot is immutable'); END;

CREATE TRIGGER submission_network_domains_insert_draft_only
BEFORE INSERT ON submission_network_domains
WHEN (SELECT state FROM submissions WHERE submission_id=NEW.submission_id) != 'draft'
BEGIN SELECT RAISE(ABORT, 'submitted snapshot is immutable'); END;

CREATE TRIGGER submission_network_domains_update_draft_only
BEFORE UPDATE ON submission_network_domains
WHEN (SELECT state FROM submissions WHERE submission_id=OLD.submission_id) != 'draft' OR (SELECT state FROM submissions WHERE submission_id=NEW.submission_id) != 'draft'
BEGIN SELECT RAISE(ABORT, 'submitted snapshot is immutable'); END;

CREATE TRIGGER submission_network_domains_delete_draft_only
BEFORE DELETE ON submission_network_domains
WHEN (SELECT state FROM submissions WHERE submission_id=OLD.submission_id) != 'draft'
BEGIN SELECT RAISE(ABORT, 'submitted snapshot is immutable'); END;

CREATE TRIGGER submission_data_categories_insert_draft_only
BEFORE INSERT ON submission_data_categories
WHEN (SELECT state FROM submissions WHERE submission_id=NEW.submission_id) != 'draft'
BEGIN SELECT RAISE(ABORT, 'submitted snapshot is immutable'); END;

CREATE TRIGGER submission_data_categories_update_draft_only
BEFORE UPDATE ON submission_data_categories
WHEN (SELECT state FROM submissions WHERE submission_id=OLD.submission_id) != 'draft' OR (SELECT state FROM submissions WHERE submission_id=NEW.submission_id) != 'draft'
BEGIN SELECT RAISE(ABORT, 'submitted snapshot is immutable'); END;

CREATE TRIGGER submission_data_categories_delete_draft_only
BEFORE DELETE ON submission_data_categories
WHEN (SELECT state FROM submissions WHERE submission_id=OLD.submission_id) != 'draft'
BEGIN SELECT RAISE(ABORT, 'submitted snapshot is immutable'); END;

CREATE TRIGGER submission_submit_once
BEFORE UPDATE OF state ON submissions
WHEN NEW.state='submitted' AND OLD.state!='draft'
BEGIN SELECT RAISE(ABORT, 'submission already submitted'); END;

CREATE TRIGGER submitted_snapshot_no_delete
BEFORE DELETE ON submissions WHEN OLD.state!='draft'
BEGIN SELECT RAISE(ABORT, 'submitted snapshot is immutable'); END;

CREATE TRIGGER submission_state_transition
BEFORE UPDATE OF state ON submissions
WHEN NEW.state!=OLD.state AND NOT (
  (OLD.state='draft' AND NEW.state='submitted') OR
  (OLD.state='submitted' AND NEW.state='in_review') OR
  (OLD.state='in_review' AND NEW.state IN ('approved','changes_required','more_information_required','rejected')) OR
  (OLD.state='more_information_required' AND NEW.state='in_review')
)
BEGIN SELECT RAISE(ABORT, 'invalid submission state transition'); END;
