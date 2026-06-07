CREATE TABLE IF NOT EXISTS teams (
    team_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (created_by) REFERENCES developers(developer_id)
);

CREATE TABLE IF NOT EXISTS team_members (
    team_id TEXT NOT NULL,
    developer_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('owner', 'admin', 'developer', 'viewer')),
    joined_at TEXT NOT NULL,
    PRIMARY KEY (team_id, developer_id),
    FOREIGN KEY (team_id) REFERENCES teams(team_id),
    FOREIGN KEY (developer_id) REFERENCES developers(developer_id)
);

CREATE INDEX IF NOT EXISTS idx_team_members_developer_id
    ON team_members(developer_id);

CREATE INDEX IF NOT EXISTS idx_team_members_team_id
    ON team_members(team_id);

CREATE TABLE IF NOT EXISTS app_team_assignments (
    bundle_id TEXT PRIMARY KEY,
    team_id TEXT NOT NULL,
    assigned_at TEXT NOT NULL,
    FOREIGN KEY (bundle_id) REFERENCES developer_apps(bundle_id),
    FOREIGN KEY (team_id) REFERENCES teams(team_id)
);

CREATE INDEX IF NOT EXISTS idx_app_team_assignments_team_id
    ON app_team_assignments(team_id);