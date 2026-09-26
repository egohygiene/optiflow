-- Additive evidence only: old review plans and scan runs gain no authority.
CREATE TABLE execution_plans (
    fingerprint TEXT PRIMARY KEY,
    document_json TEXT NOT NULL
);
CREATE TABLE execution_approvals (
    authorization_id TEXT PRIMARY KEY,
    plan_fingerprint TEXT NOT NULL REFERENCES execution_plans(fingerprint),
    document_json TEXT NOT NULL
);
CREATE TABLE execution_runs (
    run_id TEXT PRIMARY KEY,
    plan_fingerprint TEXT NOT NULL REFERENCES execution_plans(fingerprint),
    authorization_id TEXT NOT NULL REFERENCES execution_approvals(authorization_id),
    status TEXT NOT NULL CHECK (status IN ('validating', 'validated', 'rejected', 'interrupted')),
    document_json TEXT NOT NULL
);
INSERT INTO schema_migrations (version, applied_at)
VALUES (6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
