ALTER TABLE releases ADD COLUMN developer_certificate_subject_key_id TEXT NOT NULL DEFAULT '';
ALTER TABLE releases ADD COLUMN developer_certificate_developer_id TEXT NOT NULL DEFAULT '';
ALTER TABLE releases ADD COLUMN developer_certificate_issuer_key_id TEXT NOT NULL DEFAULT '';
ALTER TABLE releases ADD COLUMN developer_certificate_issuer_public_key TEXT NOT NULL DEFAULT '';
ALTER TABLE releases ADD COLUMN developer_certificate_issuance_source TEXT NOT NULL DEFAULT 'legacy_root';
