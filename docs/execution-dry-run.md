# Approved exact-duplicate dry runs

This development feature implements #90. It is **not included in the immutable
v0.1.1 release**. Build the current source to use it. It never moves, deletes,
quarantines, replaces, or restores a source file. Live `apply` remains disabled.

## Select, review, approve, validate

An inventory review plan's `keep_path` is only a deterministic suggestion.
It carries no execution authority and cannot be passed to this command.
Select the keeper and each candidate yourself. The new plan captures fresh
identity, metadata, complete hashes, and direct byte comparison for those paths.
A plan can select one keeper and several candidates; each candidate is a
separate, sequential action. Neither creation nor approval changes source media.

Use existing directories: source roots, quarantine, and local state must be
separate. State and quarantine must be outside every declared source root and
must not contain one another. Evidence output also stays outside source roots
and quarantine. Use synthetic copies for a first trial.

```bash
# Create only tool-owned directories outside your collection.
mkdir -p /local/optiflow-state /drive/optiflow-quarantine /local/optiflow-evidence

optiflow --no-config --state-directory /local/optiflow-state \
  plan execution \
  --root /drive/collection \
  --subtree /drive/collection/selected-folder \
  --keep /drive/collection/selected-folder/chosen-original.bin \
  --candidate /drive/collection/selected-folder/chosen-duplicate.bin \
  --quarantine /drive/optiflow-quarantine \
  --max-actions 1 --max-in-flight-bytes 1073741824 \
  --reserve-bytes 268435456 \
  --output /local/optiflow-evidence/execution-plan.json

# Read the complete plan. Copy its exact fingerprint into this explicit approval.
optiflow --no-config plan approve \
  --plan /local/optiflow-evidence/execution-plan.json \
  --fingerprint REVIEWED_64_CHARACTER_FINGERPRINT \
  --approved-by "local operator" \
  --output /local/optiflow-evidence/approval.json

optiflow --no-config --state-directory /local/optiflow-state --json \
  apply --plan /local/optiflow-evidence/execution-plan.json \
  --approval /local/optiflow-evidence/approval.json --dry-run
```

Both documents are create-only: existing destinations are refused. To change a
selection, policy, scope, location, or limit, create and review a new plan and
approval. Omitting `--approval` fails with `execution_approval_required`.
Omitting `--dry-run` fails with `execution_unsupported` before starting a run.

Roots and subtrees can be repeated; overlapping roots or subtrees are rejected.
If no subtree is given, each declared root is the allowed subtree. The default
bounds are 100 actions (absolute v1 limit: 1,000), one in-flight candidate,
1 GiB in-flight candidate bytes, and a 256 MiB reserve on **each** affected
filesystem. Every value is in the approved plan. A zero reserve is invalid.
The in-flight byte bound is the selected candidate's logical size, not a claim
about process RSS. Comparison uses fixed-size buffers.

## What is checked

- Strict schema versions, duplicate/unknown JSON keys, document size, semantic
  invariants, plan fingerprint, approval fingerprint, and policy compatibility.
- Every selected file's filesystem/device and object identity, link count,
  size, modification/change times, ownership and mode, before and after reads.
- Complete BLAKE3 hashes and independent byte comparison through the same
  read-only handles. Historical scan caches are never used for this proof.
- Root, subtree, parent, state and quarantine directory identities; no-follow
  opens for every canonical path component; no nested filesystem traversal.
- Single-link regular files only. Aliases, substituted links, FIFOs, duplicate
  actions, keeper/candidate cycles, and paths outside approved scope fail closed.
- Readable sources, directory write/search access, read-only volume flags,
  immutable/append-only inode flags, and a vacant quarantine namespace.
  Sticky directories and filesystems lacking required flag evidence are refused.
- Measurable free space available to the calling user, allocation unit,
  supported destination name length, same/cross-filesystem topology, retained
  copies, journal/metadata budgets, overflow, and reserve.

After content validation, all selected identities and capacity are checked
again. There is no successful result with unchecked selected actions. Failure
has stable typed diagnostics; changed state normally exits 5, unavailable
capability/space exits 4, invalid authority/scope exits 2, and interruption exits
130/143. Human output contains the same execution evidence as JSON output.

## Storage evidence

Capacity is accounted per filesystem without crediting hypothetical savings.
Existing sources are reported as a baseline. Same-filesystem quarantine would
retain their data allocation. Cross-filesystem quarantine projects the sum of
**all retained copies**, each rounded to the destination allocation unit. The
current temporary copy becomes its retained quarantine object, so it is not
counted twice. The plan reserves 16 MiB for journal growth plus conservative
64 KiB metadata budgets per affected action/filesystem, then adds the approved
free-space floor. Arithmetic overflow or an unavailable measurement blocks
validation. Budgets are projections, not a reservation against other processes.

`selected_logical_bytes` describes the selected duplicate bytes.
`immediate_logical_reclaimed_bytes` is always zero in a dry run.
`physical_reclaimed_bytes` is always null and `physical_status` is `unknown`:
shared extents, snapshots, compression and deferred deletion prevent a defensible
physical-savings claim. Quarantining a file will not itself prove reclamation.

## Approval and evidence identity

`execution-v1.schema.json` defines seven independent v1 document kinds: plan,
approval, run, attempt, validation, commit and recovery. Fingerprints are BLAKE3
of the UTF-8 JSON body with lexically sorted object keys, compact separators,
and preserved array order. Whitespace and object key ordering in the input file
do not change identity. Every declared plan body field is included. The evidence
policy fingerprint excludes presentation choices, so switching `--json` does
not invalidate approval. Changing evidence policy does.

Approval is a separate document naming the complete plan fingerprint and an
explicit `quarantine_exact_duplicates` authority scope. Its own deterministic
fingerprint binds that scope, operator label, timestamp and plan. It is a local
audit record, **not a signature or authentication service**. Protect the state
and evidence directories from concurrent/untrusted writers. The runtime still
has no source-mutation capability, even with an approved plan.

## Durable state and recovery

Migration 0006 adds immutable plan/approval rows and versioned execution-run
JSON to the existing local SQLite store. Historical scans and review plans
remain unchanged and receive no approval. Future document versions are rejected;
future state versions are refused by this execution reader.

The journal uses full SQLite synchronization and a process-held OS lock.
A run and each attempt's validating state are durably recorded before checking
content. Terminal evidence is saved after validation. Completed runs cannot be
rewritten; repeating the dry run creates a new run ID. Use the returned JSON as
the execution report; the library's `execution::load_execution` reads the same
saved evidence by run ID without migrating the database.

A hard exit leaves a `validating` record, never a commit. The next execution
that acquires the lock marks abandoned runs/attempts `interrupted`, records
`interrupted_dry_run` recovery, and requires fresh validation. Opening a scan or
another active process cannot steal the run. A failure before safe journal
initialization returns a typed diagnostic without fabricating a durable run.

## Guarantees that remain unsupported

This is point-in-time evidence, not a reservation or a future commit token.
Reads may update access times. No source or quarantine content is written.
The tool writes only its external evidence/state. It cannot guarantee safety
against concurrent hostile changes to the operator-owned state directory.

The dry run cannot test rename atomicity, synchronization after a future write,
copy fidelity of every filesystem property, restore, quotas, remote durability,
or capacity consumed by other processes. Cross-filesystem mutation is never
atomic. No mutation recovery guarantee is claimed: every commit record says
`not_started`, zero actions, and `source_mutated: false`; the only current
recovery guarantee is `no_source_mutation`.

#91 must revalidate immediately before each consequential step, establish its
metadata-preservation and recovery guarantees, and refuse unsupported hosts.
These v1 commit/recovery contracts intentionally cannot encode a successful
mutation. Any later mutation authority needs an explicit contract evolution.
