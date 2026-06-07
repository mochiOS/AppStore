ALTER TABLE developer_releases
    ADD COLUMN package_path TEXT;

ALTER TABLE developer_releases
    ADD COLUMN package_size INTEGER;

ALTER TABLE developer_releases
    ADD COLUMN changelog TEXT;

ALTER TABLE developer_releases
    ADD COLUMN review_message TEXT;

ALTER TABLE developer_releases
    ADD COLUMN submitted_at TEXT;

ALTER TABLE developer_releases
    ADD COLUMN reviewed_at TEXT;

ALTER TABLE developer_releases
    ADD COLUMN reviewed_by TEXT;

ALTER TABLE developer_releases
    ADD COLUMN published_at TEXT;