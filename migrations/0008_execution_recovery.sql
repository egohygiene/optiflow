-- Append-only v3 operator transitions. v1 previews and v2 mutation rows stay intact.
CREATE TABLE execution_recovery_events (
    sequence INTEGER PRIMARY KEY,
    event_id TEXT NOT NULL UNIQUE,
    run_id TEXT NOT NULL REFERENCES execution_mutation_runs(run_id),
    operation_id TEXT NOT NULL,
    action_id TEXT,
    operation TEXT NOT NULL CHECK (operation IN ('apply', 'resume', 'restore', 'cleanup')),
    phase TEXT NOT NULL,
    document_json TEXT NOT NULL
);
CREATE INDEX execution_recovery_by_run ON execution_recovery_events(run_id, sequence);
CREATE TRIGGER execution_recovery_no_update BEFORE UPDATE ON execution_recovery_events
BEGIN SELECT RAISE(ABORT, 'recovery events are immutable'); END;
CREATE TRIGGER execution_recovery_no_delete BEFORE DELETE ON execution_recovery_events
BEGIN SELECT RAISE(ABORT, 'recovery events are immutable'); END;
INSERT INTO schema_migrations (version, applied_at)
VALUES (8, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
