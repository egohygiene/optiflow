# Execution v1 compatibility fixtures

Frozen, synthetic wire examples. Paths, identities, timestamps, hashes and
capacity are synthetic; these are not approved plans for real files. The plan
and approval fingerprints are BLAKE3 of their sorted, compact JSON bodies.

`tests/execution.rs` checks every document, fingerprint, unknown/future version
refusal, and the inability of v1 commit records to claim source mutation.
Migration 0006 tests separately prove additive upgrades, idempotence and rollback.
