-- Separate replacement family; no reinterpretation of exact-quarantine rows.
CREATE TABLE execution_replacement_events (
    plan_fingerprint TEXT NOT NULL,
    sequence INTEGER NOT NULL CHECK(sequence >= 0),
    phase TEXT NOT NULL,
    document_json TEXT NOT NULL,
    PRIMARY KEY(plan_fingerprint, sequence)
);
CREATE TRIGGER execution_replacement_no_update BEFORE UPDATE ON execution_replacement_events
BEGIN SELECT RAISE(ABORT, 'replacement events are immutable'); END;
CREATE TRIGGER execution_replacement_no_delete BEFORE DELETE ON execution_replacement_events
BEGIN SELECT RAISE(ABORT, 'replacement events are immutable'); END;
INSERT INTO schema_migrations(version,applied_at)
VALUES(10,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
