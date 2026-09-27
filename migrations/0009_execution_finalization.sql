-- Separate irreversible v4 evidence. v1-v3 rows and guarantees are unchanged.
CREATE TABLE execution_finalization_events (
    sequence INTEGER PRIMARY KEY,
    event_id TEXT NOT NULL UNIQUE,
    run_id TEXT NOT NULL REFERENCES execution_mutation_runs(run_id),
    action_id TEXT NOT NULL,
    phase TEXT NOT NULL CHECK (phase IN ('removal_pending', 'removed')),
    preview_fingerprint TEXT NOT NULL,
    authorization_id TEXT NOT NULL,
    document_json TEXT NOT NULL,
    UNIQUE (run_id, action_id, phase)
);
CREATE INDEX execution_finalization_by_run ON execution_finalization_events(run_id, sequence);
CREATE TRIGGER execution_finalization_no_update BEFORE UPDATE ON execution_finalization_events
BEGIN SELECT RAISE(ABORT, 'finalization events are immutable'); END;
CREATE TRIGGER execution_finalization_no_delete BEFORE DELETE ON execution_finalization_events
BEGIN SELECT RAISE(ABORT, 'finalization events are immutable'); END;
INSERT INTO schema_migrations (version, applied_at)
VALUES (9, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
