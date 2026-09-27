# Bounded exact-duplicate quarantine (development source)

This #91 implementation is not in the immutable v0.1.1 release. Live apply is
supported on Linux where the required no-replace rename, file identity, inode
flags, extended attributes, directory synchronization, and capacity checks are
available. On other hosts it refuses before mutation. Test with synthetic files
first. No plan is ever applied to user media by the repository tests.

## Authority and execution

Create an explicit execution plan and a separate fingerprint-bound approval as
described in [the preview protocol](execution-dry-run.md). Review the entire
plan, including root/subtree paths, keeper and candidate selection, quarantine
directory, max actions, one in-flight file, byte bound, and per-filesystem
reserve. Approval is a local audit record, not a cryptographic signature. The
review plan's suggested keeper is never authority.

After `apply --dry-run`, invoke live apply only when you intend to move the
approved candidate paths:

```bash
optiflow --no-config --state-directory /local/optiflow-state --json \
  apply --plan /local/optiflow-evidence/execution-plan.json \
  --approval /local/optiflow-evidence/approval.json
```

The command checks authority, policy, current directories, scope, topology,
capacity, and namespace before acquiring the exclusive journal lock. It then
rechecks capacity and validates each selected pair with full hashes and direct
byte comparison immediately before that action. It checks the current source
handle and path again before a move or removal. Actions execute sequentially;
the plan allows at most 1,000 actions and exactly one in-flight candidate,
bounded by the approved `max_in_flight_bytes`. Each approved root and subtree
is a hard boundary. A plan fingerprint owns one create-only namespace at
`QUARANTINE/PLAN_FINGERPRINT/`; each destination is named by its action ID.
An occupied namespace is refused, including after an interrupted run. The
separately versioned [#92 recovery protocol](execution-recovery.md) supplies
operator-initiated status, bounded resume, restore and empty-namespace cleanup
for new runs with a pre-mutation v3 authority binding.

On the same filesystem, a no-replace atomic rename preserves the file object
and its ownership, permissions, timestamps, extended attributes and other
inode properties. The source and destination directories are synchronized.
Cross-filesystem execution creates an exclusive temporary file in quarantine,
copies bounded bytes, preserves and checks owner, mode, access/modification
times and enumerated extended attributes (including ACLs), then synchronizes
and verifies full content hash and independent byte equality. It refuses
unreadable, oversized or unpreservable attributes and special inode flags.
After a no-replace rename commits the verified destination and synchronizes
its directory, it revalidates the pair, properties and remaining capacity
before unlinking the source and synchronizing its directory. Cross-filesystem
copy plus unlink is **not atomic**. Creation/change timestamps and filesystem
allocation layout are not portable copy properties; a copy has a new identity.

## Journal and recovery

Migration 0007 adds `execution_mutation_runs` with strict
`optiflow.execution-mutation.v2` records. It leaves the dry-run v1 schema and
rows untouched: a v1 commit still cannot represent source mutation. SQLite
uses full synchronization under an exclusive OS lock. The run and first
attempt are recorded before creating a namespace. Each action records its
phase before and after rename, temporary copy, destination commitment, and
source removal. A committed action requires a durable, verified destination
and an absent source. A failure after any attempted filesystem step remains
`interrupted`; it never becomes a completed run. The next journal opener
classifies an abandoned `running` mutation as `interrupted` without changing
files. Retrying the same plan first recovers its abandoned record, then refuses
the occupied namespace. `execution::load_mutation(state, run_id)` reads the
same stored evidence without changing state.

For an interrupted action, use the recorded `source`, `temporary`,
`destination`, `phase`, and plan binding. Keep the namespace intact. Inspect
the source and both quarantine paths with no-follow opens, compare identities,
complete hashes, bytes and required metadata against the plan, and determine
which filesystem operations persisted. `rename_pending` may mean either the
source still exists or it moved; `source_removal_pending` may mean either the
source remains alongside the durable copy or it was removed. If both paths,
neither path, or a replaced object is observed, stop for manual investigation.
For an unambiguous same-filesystem move, the original file object can be moved
back with an exclusive no-replace rename to a vacant original path. For a
cross-filesystem copy, verify the quarantine copy and restore its properties
before considering a return copy. The [recovery commands](execution-recovery.md)
retain ambiguous work for inspection and never promote historical v2 evidence
into authority.

`selected_logical_bytes` describes approved duplicate content. Immediate
logical reclaimed bytes remain zero because quarantine retains the data.
Physical reclaimed bytes remain `null` with status `unknown`; neither a move
nor a source unlink proves allocation freed in the presence of snapshots,
shared extents, compression and delayed reclaim.

## Limits of the guarantee

The journal states the last **durably recorded** phase, not a promise that a
filesystem step following it never occurred. Another process can change
files or consume space after any check; this protocol does not provide an
atomic compare-and-rename or compare-and-unlink against a hostile concurrent
writer. The operator-owned state and quarantine directories must be protected
from untrusted concurrent changes. Network filesystems, quotas and remote
power-loss durability are not qualified. An unsupported property or filesystem
operation causes an explicit refusal and leaves any temporary copy and journal
for inspection. There is no permanent deletion, encoding or automatic cleanup.
