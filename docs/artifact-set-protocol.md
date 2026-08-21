# Artifact-Set Commit Protocol

## Publication contract

New v5 scan and plan documents are not considered published merely because a
JSON file exists. Publication requires a valid `optiflow.artifact-set.v1`
marker whose member list, schemas, byte lengths, BLAKE3-256 digests, run
identity, and artifact-set identity all agree with the files on disk.

Readers classify a set as exactly one of:

| State | Meaning | Reader behavior |
| --- | --- | --- |
| `committed` | The marker is supported and every declared member validates | The artifacts may be consumed |
| `incomplete` | A marker or member is missing, unreadable, truncated, or has the wrong digest | Refuse the set with `artifact_set_incomplete` |
| `incompatible` | The marker schema, structure, kind, member schema, or identity binding is unsupported or contradictory | Refuse the set with `artifact_set_incompatible` |

Historical v1-v4 reports predate this protocol and remain readable without a
marker. Newly emitted v5 reports fail closed when their marker is absent.

## Scan sets

A scan publishes `effective-policy.json`, `run.json`, and `report.json` as one
directory set:

1. Create a uniquely named sibling staging directory under `runs/`.
2. Serialize already contract-validated documents into that directory.
3. Flush and `fsync` every member.
4. Build and validate `artifact-set.json` from the exact serialized bytes.
5. Flush and `fsync` the marker, then `fsync` the staging directory.
6. Rename the staging directory to `runs/<run-id>` in one filesystem operation.
7. `fsync` the `runs/` parent directory.
8. Promote observations and groups in one SQLite transaction.

The run and embedded report run both carry the marker's `artifact_set_id`.
Extra files created later, such as a review plan, are not members of the sealed
scan set and do not alter it.

The directory rename is the visibility boundary: before it, the final run path
does not exist; after it, every member and the marker exist together. If a
process stops after the rename but before SQLite promotion, the next state-store
open verifies the committed set and idempotently reconstructs the completed
database row from `run.json` and `report.json`.

## Plan sets

The public `--output <file>` contract is retained for plans, so publication
cannot use a directory rename. Instead OptiFlow uses a recoverable marker
handshake in the output directory:

1. Write and `fsync` a uniquely named staged plan.
2. Write and `fsync` a hidden pending marker containing the final plan digest.
3. `fsync` the output directory.
4. Rename the staged plan to the requested output and `fsync` the directory.
5. Rename the pending marker to `<output>.artifact-set.json` and `fsync` the
   directory again.

OptiFlow readers never accept the plan between steps 4 and 5. If execution
stops in that window, recovery verifies the published plan against the pending
marker before atomically promoting the marker. Orphaned staging files and
pending markers that have no published plan are discarded. Existing committed,
incomplete, incompatible, or unrecognized output files are never overwritten.

A plan marker records the source scan set identifier when one exists. Plans
derived from historical pre-marker reports record a null source set explicitly.

## Failure and recovery matrix

| Failure point | Visible state | Recovery |
| --- | --- | --- |
| Member serialization or validation | No final set | Mark the scan failed; no artifacts are advertised |
| Disk full while staging | Reserved staging only | Remove staging on the next state open |
| Process stop before scan directory rename | No final set | Remove staging on the next state open |
| Process stop after scan directory rename | Complete committed set | Reconcile a still-running SQLite row from the set |
| Process stop before plan output rename | Reserved staging/pending state only | Discard orphaned reserved files |
| Process stop after plan output rename | Incomplete plan set with pending marker | Verify and promote the marker |
| Missing or changed committed member | Incomplete set | Refuse; never reconstruct or guess member content |
| Unknown or contradictory marker | Incompatible set | Refuse; require an explicit compatible reader or repair |

Crash and disk-full paths are exercised through deterministic fault injection.
Evidence covers a crash after a staged member, disk exhaustion during a member
write, a crash immediately after the scan directory rename, and a crash after
the plan rename but before marker promotion.

## Durability boundary

On Linux and macOS, regular files and relevant directories are explicitly
`fsync`ed. A successful return means member data, the marker, and the directory
entry rename have all crossed the operating-system durability calls available
through the safe Rust standard library. Hardware, filesystem, mount, network,
and virtualization layers may still weaken those guarantees; OptiFlow does not
claim stronger persistence than the underlying stack provides.

If the final parent-directory sync fails after a rename, the set may be visible
but its crash durability is uncertain. OptiFlow returns a blocking failure and
leaves the scan row recoverable instead of claiming success. A later reader may
accept it only if the complete marker and every member still verify.

Directory `fsync` is not guaranteed by the current non-Unix fallback. Windows
is not a supported v0.1.x platform, and the protocol documents rather than
overstates that boundary.

## Verification map

| Invariant | Acceptance evidence | Guard | Recovery or refusal |
| --- | --- | --- | --- |
| Related outputs publish together | Scan set has three required members and one marker | Staging plus directory rename | Remove staging or reconcile committed set |
| Marker describes exact bytes | Size and BLAKE3-256 for every member | Verification before every current-set read | `incomplete` on mismatch |
| Schemas and identities agree | v5 document IDs and member schemas bind to marker | Contract and semantic validation | `incompatible` on contradiction |
| Plan output remains file-compatible | Plan plus sidecar marker at existing path | Pending-marker handshake | Promote verified pending marker |
| No silent overwrite | Committed and unknown destinations are refused | Create-new staging and destination checks | Caller selects a new output |
