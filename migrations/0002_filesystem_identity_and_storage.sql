-- Migration 0002: filesystem identity and storage accounting
-- Adds stable identity, allocation, and hard-link coverage columns to
-- the observations table.  Compatible with v0.1.0 databases that contain
-- only the 0001 schema.

-- Record this migration.
INSERT OR IGNORE INTO schema_migrations (version, applied_at)
VALUES (2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

-- Add new columns to the observations table.
-- Existing rows will have NULL for all new columns, which is correct
-- for data imported from optiflow v0.1.0 (identity was unavailable).
ALTER TABLE observations ADD COLUMN filesystem_id TEXT;
ALTER TABLE observations ADD COLUMN file_id TEXT;
ALTER TABLE observations ADD COLUMN reported_link_count INTEGER;
ALTER TABLE observations ADD COLUMN allocated_size_bytes INTEGER;
ALTER TABLE observations ADD COLUMN identity_available INTEGER NOT NULL DEFAULT 0;
ALTER TABLE observations ADD COLUMN allocation_available INTEGER NOT NULL DEFAULT 0;

-- Index that allows fast lookup of all observations sharing the same
-- stable filesystem identity (used for hard-link group detection).
CREATE INDEX IF NOT EXISTS observations_identity_idx
ON observations(filesystem_id, file_id)
WHERE filesystem_id IS NOT NULL AND file_id IS NOT NULL;
