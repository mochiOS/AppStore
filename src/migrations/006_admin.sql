CREATE TABLE IF NOT EXISTS admin_developers (
    developer_id TEXT PRIMARY KEY,
    role TEXT NOT NULL DEFAULT 'admin' CHECK(role IN ('admin', 'owner')),
    created_at TEXT NOT NULL,
    FOREIGN KEY (developer_id) REFERENCES developers(developer_id)
);