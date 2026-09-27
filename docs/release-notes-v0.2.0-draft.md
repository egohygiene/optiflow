---
title: v0.2.0 release notes draft
description: Review-only release boundary; publication awaits native disposable-volume qualification.
---

# `v0.2.0` release notes — draft, not published

The public release remains `v0.1.1`. These notes describe development-source
behavior and must be checked against the exact final release source, archives
and native pilot receipts before publication.

## Intended bounded capability

- Select exact keeper and candidate paths in a new execution plan. The
  read-only review plan's suggested `keep_path` is never write authority.
  A separate local fingerprint-bound approval enables a bounded dry run or
  live quarantine; it is an audit record, not a cryptographic signature.
- On supported Linux filesystems, revalidate complete hashes, direct bytes,
  path identities, properties, scope and reserves before each sequential
  action. Same-filesystem no-replace rename is atomic where supported;
  cross-filesystem copy, verification and source unlink is not atomic.
- Durable journals support read-only status, bounded resume of untouched
  actions, explicit restore and empty owned-namespace cleanup. Ambiguous
  pending steps require inspection. Cross-filesystem restore retains the
  quarantine copy.
- An irreversible finalization has its own preview and separate authorization.
  It can remove only a selected retained quarantine copy **after** a completed
  cross-filesystem restore, while re-proving the surviving source and keeper.
  An unlink pending without a completion event is never reported committed.

## Storage evidence

Logical selected bytes, quarantined bytes, observed allocated blocks and
per-volume free-space changes have different meanings. Quarantine retains
content; cross-volume movement may increase available space on the source
volume while consuming it on the quarantine volume. Restoring a source before
finalizing its retained copy returns that source volume to its previous
logical occupancy. Shared extent bytes and causally proven physical savings
remain unknown; no aggregate reclaimed-space promise is made.

## Exclusions and release gates

There is no direct deletion at an original source path, automatic expiry,
media encoding, PNG replacement, universal removable-filesystem support, or
cryptographic operator approval. Development mutation refuses on macOS. An
APFS disposable-volume pilot, a portability-relevant external-format pilot,
failure/interruption/disconnect proof, clean installation and upgrade from
`v0.1.x` on all three supported binary targets, and independent signed-bundle
verification are still required. The release workflow rejects new sources and
versions until a separate native mutation qualification contract is implemented.

Back up the complete state directory and disposable source before an upgrade.
Stop the process before rollback; restore the pre-upgrade state at its original
absolute path and run the previously verified binary. Do not open a newly
migrated state with that older binary. A published release is immutable; a
correction uses a new reviewed patch version. See the
[mutation operator guide](external-drive-mutation-pilot.md).
