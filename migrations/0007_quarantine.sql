-- v1 dry-run rows retain their no-mutation invariant. Historical evidence gains no authority.
CREATE TABLE execution_mutation_runs (
    run_id TEXT PRIMARY KEY,
    plan_fingerprint TEXT NOT NULL REFERENCES execution_plans(fingerprint),
    authorization_id TEXT NOT NULL REFERENCES execution_approvals(authorization_id),
    status TEXT NOT NULL CHECK (status IN ('running', 'completed', 'rejected', 'interrupted')),
    document_json TEXT NOT NULL
);
INSERT INTO schema_migrations (version, applied_at)
VALUES (7, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
