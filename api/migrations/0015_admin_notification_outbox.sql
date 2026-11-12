CREATE TABLE admin_notification_outbox (
    id TEXT PRIMARY KEY,
    payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at INTEGER NOT NULL
);
CREATE INDEX admin_notification_outbox_due ON admin_notification_outbox(next_attempt_at, id);

CREATE TRIGGER submission_review_requires_in_review
BEFORE INSERT ON submission_reviews
WHEN COALESCE((SELECT state FROM submissions WHERE submission_id=NEW.submission_id),'') != 'in_review'
BEGIN SELECT RAISE(ABORT, 'submission is no longer in review'); END;

CREATE TRIGGER submission_reply_requires_information_request
BEFORE INSERT ON submission_messages
WHEN NEW.author_role='developer' AND
    COALESCE((SELECT state FROM submissions WHERE submission_id=NEW.submission_id),'') != 'more_information_required'
BEGIN SELECT RAISE(ABORT, 'submission is no longer waiting for information'); END;
