-- Migration 0003: observation stability and evidence validity (v3)
-- Adds typed stability and evidence-validity columns to the observations table.
-- Compatible with v0.1.x databases that contain only the 0001 and 0002 schema.

-- Record this migration.
INSERT OR IGNORE INTO schema_migrations (version, applied_at)
VALUES (3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

-- Add stability columns.  Existing rows default to 'stable'/'current' so that
-- observations imported from optiflow v2 artifacts remain usable.
ALTER TABLE observations ADD COLUMN observation_stability TEXT NOT NULL DEFAULT 'stable';
ALTER TABLE observations ADD COLUMN evidence_validity TEXT NOT NULL DEFAULT 'current';
ALTER TABLE observations ADD COLUMN attempt_count INTEGER NOT NULL DEFAULT 1;

-- Index that allows fast lookup of all unstable observations in a run.
CREATE INDEX IF NOT EXISTS observations_stability_idx
ON observations(run_id, evidence_validity)
WHERE evidence_validity != 'current';
