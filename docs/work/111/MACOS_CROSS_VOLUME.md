# #111 — macOS cross-volume metadata prerequisite

Status: proposed implementation contract, 2026-10-06. This packet accompanies
draft [PR #122](https://github.com/egohygiene/optiflow/pull/122). It does **not**
enable macOS cross-volume quarantine or restoration. Tests, compilation and
native qualification remain deferred under the [handoff](HANDOFF.md).

## Finding and current behavior

A same-volume rename retains the inode and its ACL. A cross-volume copy creates
a new inode, so observing ordinary extended attributes is insufficient to prove
that its Darwin access-control list was preserved. The existing Mac property
snapshot does not observe ACL entries independently. An unreadable or unknown
ACL cannot be treated as an empty ACL.

The locked rustix 1.1.5 API exposes `fcopyfile` and its state lifetime through
unsafe functions. Optiflow's `Cargo.toml` forbids unsafe code. The reviewed exacl
0.13 API accepts paths rather than the already-bound file handles required by
the transaction. No suitable safe descriptor-based ACL reader/writer was found
in this bounded investigation. This is a dependency/API gap, not proof that
macOS cannot support the operation.

The next checkpoint now supplies a [concrete exacl extension proposal](acl-adapter/README.md)
against a pinned upstream revision. It is an unvalidated patch artifact, not an
installed dependency, accepted upstream change or enabled transfer path.

All Mac plans containing a cross-volume action continue to fail with
`ExecutionUnsupported` before opening writable execution state or creating a
namespace. The entire plan refuses, including a same-volume action followed by
a cross-volume action. The diagnostic now names the missing ACL backend.
Existing Linux/Mac property fingerprint encodings are not reinterpreted by
this prerequisite checkpoint. Separate recovery identity hardening is described
in the [handoff](HANDOFF.md).

## Required adapter contract

The next implementation needs a reviewed dependency exposing a safe API over
borrowed file descriptors. A local unsafe shim, `/dev/fd` pathname workaround,
or subprocess that reopens paths is not an accepted substitute in this packet.
Any proposal to change that boundary requires an explicit architecture review.

| Property | Required observation and copy behavior |
| --- | --- |
| Object binding | Use the held source and exclusive destination handles; verify regular-file type, device/inode, link count and stable metadata before and after observations. Never reopen a user-controlled path to copy metadata. |
| Darwin ACL | Distinguish absent from present-empty ACLs and preserve complete ordered entries. Preserve raw principal UUIDs, allow/deny tags, rights, entry inheritance flags and ACL-level flags without resolving names or reordering entries. Refuse unknown public bits, unreadable entries or bound overflow; preserve the native ACL-header private bits that Apple's contract requires callers to retain. Independently reread the destination; a successful setter/copy call alone is insufficient. |
| ACL resource bounds | Establish finite entry/byte bounds before allocation and fingerprinting. Proposed initial bounds: 128 entries and 64 KiB encoded ACL; reject larger observations. These are proposed limits, not current runtime behavior. |
| Ownership and permissions | Preserve uid, gid and mode exactly; refuse when privilege or destination policy prevents it. Recheck after ACL installation because operations may interact. |
| Extended attributes | Retain the current 128-entry, 255-byte-name, 64-KiB-value and 1-MiB-total limits. Preserve complete values, including bounded resource forks. Reject unexplained destination-only attributes and unreadable source attributes. |
| BSD flags | Start with an explicitly observed zero-flags subset unless a reviewed descriptor setter and exact verification support more. Never silently drop immutable, append-only, compressed or unknown flags. |
| Modification and creation time | Preserve nanosecond mtime and birth time with checked signed timestamp conversion and exact read-back. `std::os::macos::fs::FileTimesExt::set_created` with `File::set_times` is available at the Rust 1.85 MSRV. Refuse values the destination cannot represent. |
| Access/change time | Content reads may advance atime; ctime and inode identity change across a copy. Do not claim those are invariants. |
| Filesystem and durability | Bind actual APFS source, destination and state handles; verify declared topology against actual devices. Retain explicit file/directory/database full-sync requirements and fail on unavailable guarantees. |

Capability and source eligibility checks must happen before any source removal;
backend availability must be checked before writable journal access. Destination
inheritance or an unpreservable attribute may only become observable after
exclusive temporary creation: leave that temporary object and recorded evidence
for inspection, with the source intact. Do not turn partial success into permission
to unlink a source.

## Transaction and recovery integration

Use the existing bounded streaming content copy and no-replace publication
sequence after the adapter is available. Observe source properties, copy bytes,
install metadata, independently compare full content and properties, then flush
the file and directory before publishing the destination. Reopen and bind the
published object to the verified temporary inode. Recheck source identity,
content, properties, capacity, namespace and destination identity before the
source-removal transition. Any failed observation or flush stops progression.
In particular, an ACL with deferred inheritance may change on its first rename.
Reject that flag in the initial copy profile or independently reobserve it
after final publication before any destructive transition.

Restoration must apply the same property contract in reverse, publish only into
a vacant original path, and retain the quarantine copy. Existing ambiguity,
interruption and separate irreversible-finalization authority remain intact.
Cross-volume copy/unlink cannot be made atomic by these checks. A visible SQLite
event is recorded evidence; a later explicit device flush can still fail.

Do not add ACL bytes to the existing `macos-apfs/v1` property digest silently.
The existing same-volume Mac digest and Linux digest must keep their meaning.
Specify an explicit cross-copy property profile with schema discrimination or
an accepted migration before persisting the stronger evidence. Missing legacy
ACL evidence must never be interpreted as an empty ACL or proof of preservation.

## Acceptance before enabling cross-volume dispatch

- Review adapter license, maintenance, API safety, bounded allocation and Rust
  1.85 compatibility; pin the accepted dependency and lockfile.
- Prove nonempty allow/deny/inherited ACL round trips, empty ACLs, inheritance
  removal, owner/mode interactions, unknown rights and unreadable ACL refusal.
- Prove timestamp edge cases, nonzero flag refusal, resource-fork/xattr bounds,
  destination-only attributes and metadata changes during copy/verification.
- Prove whole-plan rejection before mutation when the backend is unavailable,
  including mixed topology and already-existing journal evidence.
- Prove copy and restore interruption at each pending/flush/rename boundary;
  substituted temporary/destination identities must not be accepted as the
  verified object, even when bytes and copied properties match.
- Run Linux regressions and native Mac compilation/behavior, then the disposable
  APFS volume and disconnect campaign. Synthetic declarations do not qualify
  actual two-volume operations or physical space reclamation.

## Inspected primary sources

- [rustix 1.1.5 descriptor-copy API and safety contract](https://github.com/bytecodealliance/rustix/blob/v1.1.5/src/fs/fcopyfile.rs).
- [exacl 0.13 public API](https://github.com/byllyfish/exacl/blob/v0.13.0/src/lib.rs).
- [Rust 1.85 Darwin file-time extensions](https://github.com/rust-lang/rust/blob/1.85.0/library/std/src/os/darwin/fs.rs).
- [Apple descriptor-copy metadata semantics](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/fcopyfile.3.html).

These sources establish available interfaces. They are not executable evidence
that Optiflow has implemented or qualified the proposed adapter.
