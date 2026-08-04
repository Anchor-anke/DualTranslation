PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    path TEXT NOT NULL UNIQUE,
    pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
    technologies_json TEXT NOT NULL DEFAULT '[]',
    file_count INTEGER NOT NULL DEFAULT 0 CHECK (file_count >= 0),
    fingerprint TEXT NOT NULL DEFAULT '',
    last_used_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_projects_recent
ON projects(pinned DESC, last_used_at DESC);

CREATE TABLE IF NOT EXISTS conversion_projects (
    conversion_id TEXT NOT NULL REFERENCES conversions(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    PRIMARY KEY (conversion_id, project_id)
);
