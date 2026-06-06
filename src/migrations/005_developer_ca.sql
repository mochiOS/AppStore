CREATE TABLE IF NOT EXISTS developer_verifications (
    developer_id TEXT PRIMARY KEY,
    verification_status TEXT NOT NULL CHECK(verification_status IN ('pending', 'verified', 'rejected')),
    note TEXT,
    verified_at TEXT,
    verified_by TEXT,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (developer_id) REFERENCES developers(developer_id)
);

CREATE TABLE IF NOT EXISTS certificate_signing_requests (
    csr_id TEXT PRIMARY KEY,
    developer_id TEXT NOT NULL,
    public_key TEXT NOT NULL,
    public_key_fingerprint TEXT NOT NULL,
    csr_pem TEXT NOT NULL,
    subject_dn TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending', 'approved', 'rejected')),
    created_at TEXT NOT NULL,
    processed_at TEXT,
    processed_by TEXT,
    rejection_reason TEXT,
    FOREIGN KEY (developer_id) REFERENCES developers(developer_id)
);

CREATE TABLE IF NOT EXISTS developer_certificates (
    certificate_id TEXT PRIMARY KEY,
    developer_id TEXT NOT NULL,
    csr_id TEXT,
    serial_number TEXT NOT NULL UNIQUE,
    certificate_pem TEXT NOT NULL,
    ca_fingerprint TEXT NOT NULL,
    public_key TEXT NOT NULL,
    public_key_fingerprint TEXT NOT NULL,
    subject_dn TEXT NOT NULL,
    issued_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('active', 'revoked', 'expired')),
    revoked_at TEXT,
    revocation_reason TEXT,
    FOREIGN KEY (developer_id) REFERENCES developers(developer_id),
    FOREIGN KEY (csr_id) REFERENCES certificate_signing_requests(csr_id)
);
