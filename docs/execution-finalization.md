# Irreversible quarantine finalization (development source)

This #96 Linux-only path permanently removes **selected retained quarantine
copies** after a completed, v3-bound cross-filesystem restore. The restored
source and the keeper must both still prove complete equality. The immutable
`v0.1.1` release remains read-only. Use synthetic files when evaluating the
development implementation; no real media plan is authorized by this document.

## Review, authorize, commit

`execution finalize` defaults to a read-only preview. It names each approved,
committed action ID explicitly and writes a create-only v4 preview document
outside the source and quarantine trees. Preview verifies the bound plan, run,
approval, current roots/subtrees, topology, reserves, ownership, mode, mtime,
extended attributes, surviving source identity, keeper, complete BLAKE3 hashes
and direct byte equality. It records the quarantine identity, allocation
observation, and a digest of the exact selected manifest. It does not arm
removal or migrate the journal.

```bash
optiflow --no-config --state-directory /local/state --json execution finalize \
  --plan /local/evidence/plan.json --approval /local/evidence/approval.json \
  --run RUN_UUID --action action-000001 --output /local/evidence/finalize-preview.json

optiflow --no-config --state-directory /local/state --json \
  execution authorize-finalization --plan /local/evidence/plan.json \
  --preview /local/evidence/finalize-preview.json \
  --fingerprint REVIEWED_PREVIEW_FINGERPRINT --approved-by OPERATOR_LABEL \
  --output /local/evidence/finalize-authorization.json

optiflow --no-config --state-directory /local/state --json execution finalize \
  --commit --plan /local/evidence/plan.json \
  --approval /local/evidence/approval.json \
  --preview /local/evidence/finalize-preview.json \
  --authorization /local/evidence/finalize-authorization.json
```

The operator must review the selected IDs, run, plan fingerprint, manifest
digest, surviving copy and irreversible notice before creating the separate
authorization. The approval and finalization authorization are local audit
records, **not** cryptographic signatures or a substitute for backups. Preserve
the plan, both authorizations, preview, run ID and journal together. No
background expiry or implicit finalization exists.

Commit takes the exclusive execution lock. Before every selected action it
rechecks current scope, identities, capacity/reserves, properties, complete
hashes, and direct bytes against the preview. The only eligible state is
`restored_retained`; same-filesystem restores already returned the original
object and have no retained copy to remove. The original source is never a
finalization target. The `removal_pending` event is durably appended before an
unlink; after unlink and parent-directory synchronization, `removed` records
an irreversible commit. Every selected action remains bounded by the original
plan's action and in-flight byte limits. A successful repeated commit only
rechecks the surviving source and adds no events.

## Interruption and investigation

```bash
optiflow --no-config --state-directory /local/state --json \
  execution status --run RUN_UUID
```

Once v4 events exist, status returns the v4 report with action states
`retained`, `pending_irreversible_inspection`, and
`finalized_irreversible`. The embedded `recovery_history` is v3 historical
evidence; its `restored_retained` label cannot grant a new restore or imply
the quarantine file remains. Without v4 events, the old v3 status shape is
preserved.

If only `removal_pending` is durable, the path may or may not have been
unlinked. Status says `attention_required`, never `removed`; commit refuses
automatic retry, and restore for that action refuses too. Preserve the journal
and all paths. Compare the recorded identities and full bytes to the keeper
and restored source before deciding any manual repair. A crash between an
unlink and a SQLite completion event cannot be made atomic across the two
stores. No manual inspection promotes an old event to authority.

`logical_bytes_removed` counts committed quarantine file lengths;
`observed_allocated_bytes_removed` sums their prior `st_blocks × 512`
observations. `shared_extent_bytes` is explicitly `null` because extent
sharing is unknown. Each event stores target available bytes before and after
the action, plus their signed difference when representable.
`physical_reclaimed_bytes` remains `null`: block sharing, delayed
allocation, snapshots and concurrent filesystem activity make an observed
free-space delta unsuitable as a causal savings claim.

Migration 0009 adds an append-only v4 event table with update/delete
protection. v1 dry runs, v2 mutation records and v3 recovery events are not
rewritten. Historical runs lacking the v3 pre-mutation binding stay
inspection-only. Filesystem or database tampering, hostile concurrent writers,
remote durability, and failures outside the local synchronization boundary
require separate operator investigation.
