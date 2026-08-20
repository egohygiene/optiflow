-- Migration 0004: lossless NativePath encoding (v4 schema)
--
-- Adds path_encoding and path_base64 columns to the observations and file_cache
-- tables.  These columns make the path encoding explicit and provide a
-- structured way to store non-UTF-8 path bytes alongside the existing TEXT path
-- column (which stores the sqlite_key form: UTF-8 string for valid UTF-8 paths,
-- or "\x00unix_bytes:" + base64 for non-UTF-8 paths).
--
-- Existing rows default to "utf8" encoding, which is correct for all data
-- written by optiflow prior to v4 (all older path storage was lossy UTF-8).
--
-- Compatible with v0.1.x databases that contain only migrations 0001–0003.

-- Record this migration.
INSERT OR IGNORE INTO schema_migrations (version, applied_at)
VALUES (4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

-- Add encoding columns to the observations table.
-- path_encoding: "utf8" for valid UTF-8 paths; "unix_bytes" for non-UTF-8.
-- path_base64:   base64-encoded raw path bytes for non-UTF-8 paths; NULL for UTF-8.
ALTER TABLE observations ADD COLUMN path_encoding TEXT NOT NULL DEFAULT 'utf8';
ALTER TABLE observations ADD COLUMN path_base64 TEXT;

-- Add the same columns to file_cache so that the analysis cache can also
-- record and look up non-UTF-8 paths losslessly.
ALTER TABLE file_cache ADD COLUMN path_encoding TEXT NOT NULL DEFAULT 'utf8';
ALTER TABLE file_cache ADD COLUMN path_base64 TEXT;
