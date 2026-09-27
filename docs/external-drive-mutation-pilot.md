---
title: Disposable-volume mutation qualification
description: Operator procedure and release gates for bounded exact-duplicate quarantine and recovery.
---

# Disposable-volume mutation qualification

This is the #93 **development-source** operator runbook. The published
`v0.1.1` remains read-only. Current live mutation, recovery and finalization
refuse on macOS, so the APFS and both macOS binary targets have **not**
qualified for `v0.2.0`. The signed release workflow refuses any new source or
version until a separately reviewed native mutation qualification is wired.
Do not use this procedure on private, irreplaceable or production media. The
[draft release notes](release-notes-v0.2.0-draft.md) describe the exact current
capability and exclusions for review.

## Prepare a disposable volume and a backup

1. Use an empty, dedicated removable test volume containing only newly
   generated disposable files. Record its device, filesystem format, mount
   path, capacity, volume identity and host architecture before writing.
   Test APFS on a native macOS host and a portability-relevant external format
   on a supported host. A directory on an internal disk or a tmpfs smoke test
   does not satisfy the real-volume gate. Record unavailable combinations as
   unqualified, rather than replacing them with simulated evidence.
2. Make an independent offline copy of the complete disposable fixture and
   the complete closed state directory. Retain verified `v0.1.1` archives and
   their signature and checksum evidence. State backup must include the SQLite
   database and artifact trees at their original absolute path; copying only
   the database is insufficient. Keep plan, approval, preview, authorization,
   run ID and journal together. Protect these files because paths and hashes
   reveal collection details.
3. Verify source, state and quarantine are three separate directories. Keep
   state and evidence on the local disk outside every source root. A
   cross-filesystem quarantine needs enough free space for the retained copy,
   metadata, journal growth and the approved reserve. Test same-filesystem
   and cross-filesystem topology separately; never rely on a copy to be an
   atomic move. Record free bytes and allocation evidence independently before
   each scenario.

For a synthetic Linux CLI smoke before the real-volume procedure, build the
development binary and supply two existing roots on **different** filesystems:

```bash
python3 scripts/smoke-exact-transaction.py \
  --binary target/debug/optiflow \
  --work-root /local/disposable-workspace \
  --cross-quarantine-root /other-filesystem/disposable-workspace \
  --output /local/evidence/synthetic-transaction.json
```

The script creates uniquely named 64 KiB synthetic files under those roots;
it never touches pre-existing collection files. It exercises both topologies,
retains its fixture directories on failure for inspection, and writes a
`optiflow.synthetic-transaction-smoke.v1` receipt with
`qualified_for_release: false`. A passing receipt is local integration
evidence, **not** APFS, removable-media, interruption or native release proof.

## Review and execute one selected duplicate

Run a read-only scan and review plan first. Its `keep_path` is a suggestion,
not approval. Choose the exact keeper and candidate from the disposable
fixture yourself. The following is a template; replace every placeholder
with the actual verified paths and retain the JSON outputs.

```bash
optiflow --no-config --state-directory /local/state --json scan --no-probe \
  /mounted/disposable/source
optiflow --no-config --state-directory /local/state --json \
  plan exact-duplicates --run SCAN_UUID \
  --output /local/evidence/review-plan.json
optiflow --no-config --state-directory /local/state --json plan execution \
  --root /mounted/disposable/source \
  --keep /mounted/disposable/source/keeper.bin \
  --candidate /mounted/disposable/source/candidate.bin \
  --quarantine /separate/quarantine --max-actions 1 \
  --max-in-flight-bytes 1048576 --reserve-bytes 268435456 \
  --output /local/evidence/execution-plan.json
optiflow --no-config --state-directory /local/state --json plan approve \
  --plan /local/evidence/execution-plan.json \
  --fingerprint REVIEWED_PLAN_FINGERPRINT --approved-by OPERATOR_LABEL \
  --output /local/evidence/approval.json
optiflow --no-config --state-directory /local/state --json apply \
  --plan /local/evidence/execution-plan.json \
  --approval /local/evidence/approval.json --dry-run
```

Check the plan's root/subtree, exact paths, topology, file identity, complete
hash and independent byte proof, ownership, mode, timestamps, extended
attributes, limits and reserve. Verify the dry-run result is `validated` with
no source commit. Copy the reviewed fingerprint into a separate approval;
the operator label is an audit record, **not** a cryptographic signature.
Recreate the plan if the mounted volume, paths, policy, capacity or files
change. Then run the same `apply` command without `--dry-run`, with an
explicit human choice to move this disposable candidate. Record its run ID.

## Inspect, recover and restore

```bash
optiflow --no-config --state-directory /local/state --json \
  execution status --run RUN_UUID
optiflow --no-config --state-directory /local/state --json \
  execution resume --run RUN_UUID --plan /local/evidence/execution-plan.json \
  --approval /local/evidence/approval.json
optiflow --no-config --state-directory /local/state --json \
  execution restore --run RUN_UUID --action action-000001 \
  --plan /local/evidence/execution-plan.json \
  --approval /local/evidence/approval.json
```

Status is recorded journal evidence, not a fresh filesystem proof. Resume
only untouched actions after current identities, bounds and capacity pass;
it refuses ambiguous or restored work. Restore must show the source has the
same complete bytes and declared properties as the keeper. A same-filesystem
restore moves the original object back. A cross-filesystem restore keeps the
verified quarantine copy; it has **not** reclaimed net storage. After every
same-filesystem action is restored, `execution cleanup` removes only an empty
owned namespace. It does not remove data files.

On interruption, use Ctrl-C once, stop issuing mutating commands, keep the
journal and all source/temp/quarantine paths, and read status. On physical
disconnect, leave the volume detached until the device and mount identity can
be checked. Reconnect to the intended volume; do not infer identity from a
reused path. A `pending`, `ambiguous` or `attention_required` transition
requires manual comparison of current identities, full hashes, bytes and
properties against the recorded plan. Do not retry a pending irreversible
unlink or mark it committed because the file appears absent. Preserve the
independent backup and seek operator review before any repair. See
[recovery](execution-recovery.md) and
[finalization](execution-finalization.md) for exact state boundaries.

## Separately authorize irreversible finalization

Only a **restored retained cross-filesystem quarantine copy** is currently
eligible. Preview the selected action, inspect its manifest and surviving
source/keeper evidence, then create a separate fingerprint-bound local
authorization. The commit permanently removes the selected quarantine copy;
it never deletes the original source path.

```bash
optiflow --no-config --state-directory /local/state --json \
  execution finalize --plan /local/evidence/execution-plan.json \
  --approval /local/evidence/approval.json --run RUN_UUID \
  --action action-000001 --output /local/evidence/finalize-preview.json
optiflow --no-config --state-directory /local/state --json \
  execution authorize-finalization \
  --plan /local/evidence/execution-plan.json \
  --preview /local/evidence/finalize-preview.json \
  --fingerprint REVIEWED_PREVIEW_FINGERPRINT --approved-by OPERATOR_LABEL \
  --output /local/evidence/finalize-authorization.json
optiflow --no-config --state-directory /local/state --json \
  execution finalize --commit --plan /local/evidence/execution-plan.json \
  --approval /local/evidence/approval.json \
  --preview /local/evidence/finalize-preview.json \
  --authorization /local/evidence/finalize-authorization.json
```

Retain status after commit, the two v4 events (`removal_pending`, `removed`),
the surviving source proof, volume free bytes before and after, and allocation
observations. `logical_bytes_removed` counts the quarantine object;
`observed_allocated_bytes_removed` is its pre-unlink block observation;
shared extents and `physical_reclaimed_bytes` remain unknown. The signed
free-space difference is an observation, not a causal saving: snapshots,
sharing, compression, deferred allocation and other processes can change it.
Restoring the source before finalizing the retained copy returns the source
volume to its original logical occupancy. Cross-volume quarantine can increase
*source-volume* available space while still consuming space on the quarantine
volume. Record both volumes and never add those figures as global reclaimed
storage.

## Release decision and rollback

Before a `v0.2.0` release, require disposable mounted-volume evidence for
APFS and a relevant external format, explicit interruption/disconnect
observations, clean installation and `v0.1.x` state upgrade/rollback on
native Linux and both macOS targets. Bind each target's receipt to its exact
archive and source revision; independently verify checksums, SPDX SBOM,
SLSA provenance, Sigstore identity and license/notices. The current read-only
release receipt is insufficient. A release decision must name any unsupported
filesystem or target and cannot treat a local smoke as a native pilot.

If an installed version is unsafe, stop using it and keep the entire state,
plans, journals and media untouched. Roll back with the verified prior binary
and the complete offline pre-upgrade state backup restored at its original
path. Never open newly migrated state with an old binary. Published tags and
assets remain immutable; distribute a newly reviewed corrective patch rather
than replacing them. No automatic retention expiry, direct original-path
deletion, image/audio/video encoding or universal filesystem compatibility is
part of this pilot.
