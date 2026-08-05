PRAGMA foreign_keys = ON;
PRAGMA journal_mode = DELETE;
PRAGMA synchronous = FULL;

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

INSERT OR IGNORE INTO schema_migrations (version, applied_at)
VALUES (1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

CREATE TABLE IF NOT EXISTS scan_runs (
    run_id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    completed_at TEXT,
    status TEXT NOT NULL,
    manifest_json TEXT,
    report_json TEXT
);

CREATE TABLE IF NOT EXISTS observations (
    observation_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES scan_runs(run_id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    content_hash TEXT,
    observation_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS observations_run_id_idx
ON observations(run_id);

CREATE INDEX IF NOT EXISTS observations_exact_candidate_idx
ON observations(size_bytes, content_hash);

CREATE TABLE IF NOT EXISTS duplicate_groups (
    group_id TEXT NOT NULL,
    run_id TEXT NOT NULL REFERENCES scan_runs(run_id) ON DELETE CASCADE,
    group_json TEXT NOT NULL,
    PRIMARY KEY (run_id, group_id)
);

CREATE TABLE IF NOT EXISTS file_cache (
    path TEXT PRIMARY KEY,
    size_bytes INTEGER NOT NULL,
    modified_unix_ns INTEGER,
    content_type TEXT,
    media_kind_json TEXT NOT NULL,
    content_hash TEXT,
    media_json TEXT,
    probe_signature TEXT,
    status_json TEXT NOT NULL,
    warnings_json TEXT NOT NULL,
    analyzed_at TEXT NOT NULL
);
