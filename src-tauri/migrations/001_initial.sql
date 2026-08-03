PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS provider_profiles (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    base_url TEXT NOT NULL,
    model TEXT NOT NULL,
    timeout_ms INTEGER NOT NULL CHECK (timeout_ms BETWEEN 1000 AND 300000),
    credential_ref TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS conversions (
    id TEXT PRIMARY KEY NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('write', 'explain')),
    original_input TEXT NOT NULL,
    sensitive INTEGER NOT NULL DEFAULT 0 CHECK (sensitive IN (0, 1)),
    target_agent TEXT NOT NULL CHECK (target_agent IN ('generic', 'cursor', 'codex')),
    output_language TEXT NOT NULL CHECK (output_language IN ('zh', 'en', 'bilingual')),
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_conversions_created_at
ON conversions(created_at DESC);

CREATE TABLE IF NOT EXISTS conversion_versions (
    id TEXT PRIMARY KEY NOT NULL,
    conversion_id TEXT NOT NULL REFERENCES conversions(id) ON DELETE CASCADE,
    version_no INTEGER NOT NULL CHECK (version_no > 0),
    structured_json TEXT NOT NULL,
    rendered_text TEXT NOT NULL,
    adjustment_text TEXT,
    changed_fields_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    UNIQUE(conversion_id, version_no)
);

CREATE INDEX IF NOT EXISTS idx_conversion_versions_conversion
ON conversion_versions(conversion_id, version_no DESC);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY NOT NULL,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS local_metric_events (
    event_name TEXT PRIMARY KEY NOT NULL,
    count INTEGER NOT NULL DEFAULT 0 CHECK (count >= 0),
    updated_at TEXT NOT NULL
);
