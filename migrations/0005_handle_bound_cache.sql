-- Migration 0005: bind reusable analysis cache entries to a full Unix file
-- state signature instead of path, size, and modification time alone.

INSERT OR IGNORE INTO schema_migrations (version, applied_at)
VALUES (5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

ALTER TABLE file_cache ADD COLUMN filesystem_id TEXT;
ALTER TABLE file_cache ADD COLUMN file_id TEXT;
ALTER TABLE file_cache ADD COLUMN changed_unix_ns INTEGER;

CREATE INDEX IF NOT EXISTS file_cache_handle_signature_idx
ON file_cache(
    path,
    filesystem_id,
    file_id,
    size_bytes,
    modified_unix_ns,
    changed_unix_ns
);
