ALTER TABLE oauth_links
    ADD COLUMN provider_username TEXT;

ALTER TABLE oauth_links
    ADD COLUMN updated_at TEXT;