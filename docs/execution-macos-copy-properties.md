# macOS cross-copy property declarations v1

Status: **unvalidated development contract**, authored under
[#111](https://github.com/egohygiene/optiflow/issues/111) in draft
[PR #122](https://github.com/egohygiene/optiflow/pull/122). This is a pure,
host-neutral library boundary for reviewing supplied property declarations.
It does not observe a file, adopt an ACL dependency, write a journal, copy data,
or grant any execution authority. Mac cross-volume plans remain refused.

## Purpose and compatibility

Cross-volume copying creates a different inode. A future native observer must
therefore supply independently observed ACL contents alongside ordinary file
properties. Historical Linux fingerprints and the same-volume Mac
`macos-apfs/v1` fingerprint omit this separate ACL contract. They keep their
existing meaning; a legacy hash must never be promoted to the stronger profile.

The separate schema is `optiflow.execution-properties-macos-cross-copy.v1` and
its required profile is `macos-apfs-cross-copy/v1`. Both identifiers contribute
to its fingerprint. The [JSON Schema](../schemas/execution-properties-macos-cross-copy-v1.schema.json)
and [synthetic example](../examples/execution-properties-macos-cross-copy-v1.json)
describe this standalone document. No existing execution v1–v4 schema,
`properties_fingerprint` field, migration or stored event is changed. The new
hex digest must not be inserted into a v3 event in place of its historical hash.
Future persistence needs an explicitly discriminated envelope or reviewed
migration together with the native observation integration.

## Library boundary

`execution::property_evidence::parse` bounds the raw JSON input before parsing,
requires JSON objects at each record boundary, then reviews the complete typed
value. Object-shape inspection is followed by typed decoding of the original
bytes so duplicate fields remain errors; the intermediate JSON value is never
the source of the typed declaration. `review` accepts an already allocated
`MacApfsCrossCopyPropertiesV1` declaration and applies the same semantic rules.
Successful review returns `ReviewedProperties`, which owns the reviewed value
and exposes immutable `snapshot()` and `fingerprint()` accessors. It is a
structural review result, **not a trusted observation or an authorization token**.
The caller can compare snapshots as data; that comparison is not a filesystem
recheck or proof that any metadata operation occurred.

There is no default, optional or inferred ACL. `absent` is an explicit claim of
a successful observation of absence; `present` with zero entries is a distinct
claim. Neither represents unavailable, unreadable, unsupported or failed
observation. Missing/null ACLs, unknown fields, duplicate object fields and
unsupported schema/profile identities refuse. A future native adapter must
propagate observation failure instead of manufacturing an `absent` value.

## Supported declaration subset

| Field | Rule |
| --- | --- |
| Raw document | At most 3 MiB before deserialization. No filesystem path reader is supplied. |
| uid / gid | Exact unsigned 32-bit identifiers, without account lookup. |
| mode | Regular-file type plus the exact permission/special bits, `0o100000` through `0o107777`. |
| mtime / birthtime | Signed 64-bit seconds and nanoseconds in `0..1_000_000_000`; negative epochs remain explicit. Destination representability is a later native obligation. |
| BSD flags | Zero only in the initial profile. Nonzero or unknown flags refuse. |
| xattrs | At most 128; exact lowercase hex names/values; names are 1–255 bytes without NUL; each value is at most 64 KiB; total value bytes are at most 1 MiB. |
| xattr ordering | Strictly increasing raw-name byte order, with no duplicate names. Input is rejected rather than silently sorted or deduplicated. |
| ACL state | Explicit `absent` or `present`; zero-entry present ACLs remain distinct. |
| ACL header flags | Preserve private low 16 bits and `NO_INHERIT`; reject `DEFER_INHERIT` and unknown public bits. |
| ACEs | At most 128 in exact order, including duplicates; retain a 16-byte raw principal as 32 lowercase hex characters, allow/deny kind, inheritance flags and supported rights. |
| ACL representation | Bounded encoded ACL document, at most 64 KiB. No name resolution, canonical ACE ordering or ACL simplification. |

The ACE flag mask is `0x0000_01f0`; the supported rights mask is
`0x01f0_3ffe`. ACL semantics and limits follow the pinned
[Apple XNU `kauth.h`](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/sys/kauth.h)
used by the separate [dependency proposal](work/111/acl-adapter/README.md).
This application-owned document does not import that proposal or freeze its
unaccepted Rust API. It preserves kernel-exposed ACL declarations; it cannot
detect on-disk state that the native kernel does not expose.

The schema enforces closed shapes, identifiers, scalar bounds and supported
bit combinations. Runtime review additionally enforces raw-input size,
aggregate xattr bytes, name ordering/uniqueness and encoded ACL size. Generic
JSON Schema success alone is insufficient; use `parse` or `review` before
computing or consuming a new-profile fingerprint.

## Fingerprint meaning

Only a fully reviewed value is fingerprinted. The encoding is compact UTF-8
JSON of the complete declaration with recursively lexicographic object keys;
array order and every scalar remain significant. The BLAKE3-256 lowercase hex
digest binds the schema and profile, ownership, mode, both timestamps, flags,
all xattrs and the complete ordered ACL. Whitespace and input object-key order
do not change the identity. Reordering or removing ACEs does.

The value deliberately contains no path, device/inode, link count, content
hash, timestamp of observation, approval, durability result or success flag.
Those are separate future host/transaction evidence. Matching property digests
alone cannot establish file identity, exact content, current metadata, APFS
topology, durability or permission to remove a source. Content reads may change
atime; ctime and identity change during copying, so they are not claimed
preservation invariants here.

## Authored coverage and remaining gate

The 20 focused synthetic integration tests cover schema/example consistency,
strict input, bounds, ordered ACL identity, absent versus empty, xattr
canonicality, timestamps, flags, deterministic fingerprints, positional-shape
refusal, duplicate fields and strict hex string endings. They are
authored and **unrun**. No compiler, tests, formatter/lint, schema checker,
native tool, patch-application or CI command was executed for this checkpoint.

To enable cross-volume copying later: qualify and adopt a reviewed safe native
ACL backend; acquire bounded observations from held descriptors; introduce
explicitly versioned journal evidence; wire content/property rechecks around
copy/restore and final publication; then execute interruption and disposable
native-volume qualification. Deferred inheritance initially refuses because
rename can change an ACL after an earlier successful copy/read-back. The
[parent handoff](work/111/HANDOFF.md) retains the complete sequence and #93's
separate release gate.
