# Descriptor ACL adapter proposal

This is a review packet for the missing dependency API in Optiflow #111.
It contains an **unvalidated upstream extension patch**, not an installed
Optiflow dependency or an enabled cross-volume mutation feature.

The patch targets exacl 0.13.0 at
[`fa36df1745a41e50ab0730546c04d86a8503e46b`](https://github.com/byllyfish/exacl/tree/fa36df1745a41e50ab0730546c04d86a8503e46b).
The upstream [descriptor API request #298](https://github.com/byllyfish/exacl/issues/298)
was open when inspected. Its requested overload/convenience API is related;
this lossless snapshot proposal is not an accepted API shape or a complete
implementation of that request. We have not contacted its maintainers, submitted the
patch there, created a fork, or selected a released dependency containing it.

## Why a separate lossless API

Copying an ACL requires the ordered entries, raw principal identifiers and
ACL-level flags to survive. The existing path-oriented convenience API is
useful for editing ACLs but is not the representation Optiflow needs for exact
metadata preservation. This proposal adds a descriptor-based snapshot/copy
boundary inside the ACL dependency, where native ownership and FFI can be
reviewed together. Optiflow would consume only a safe public API.

The application still has `unsafe_code = "forbid"`. Its `Cargo.toml`, lockfile,
mutation dispatch and existing property fingerprints are unchanged. The patch
is stored under documentation and is neither built nor linked by Optiflow.
It must not be copied into the application as a local unsafe shim.

## Proposed API and supported subset

[`exacl-fd.patch`](exacl-fd.patch) adds a public `fd_macos` module for 64-bit
macOS. It uses borrowed file descriptors and exposes no caller-facing raw
pointers. The native bridge uses public attribute-list interfaces with bounded
buffers, avoiding path reopening and the allocating ACL getter retry loop.

| API | Meaning |
| --- | --- |
| `read_fd(fd, limits)` | Check valid and supported volume extended-security capability, perform a strict ACL query, decode its bounded result, and recheck capability. Native errors and malformed returned data remain errors. |
| `AclSnapshot::Absent` | A successful strict observation on a supported volume found no kernel-exposed ACL. This is distinct from a present zero-entry ACL. |
| `AclSnapshot::Present { flags, entries }` | Exact native ACL-header flags and ordered entries containing raw 16-byte principal UUIDs, allow/deny kind, inheritance flags and rights. No account lookup, sorting or deduplication occurs. |
| `Limits { maximum_entries }` | Caller-selected bound from zero through Darwin's 128-entry profile. Native input/output buffers are bounded; system-call latency is not. |
| `replace_fd(fd, snapshot, limits)` | Validate the requested snapshot and observe the destination, apply only ACL metadata, then obtain a fresh snapshot and require exact equality. Failure can occur after the destination changed. |

Unknown public flag/right bits, audit/alarm ACE kinds, invalid references,
truncated buffers, excess entries and unavailable volume capability refuse.
The low 16 native ACL-header bits are preserved without interpretation, as
Apple requires. This API reports **kernel-exposed ACL state**. The kernel can
normalize some malformed on-disk security data to absence; the adapter cannot
detect storage corruption that the kernel does not expose.

No serialized wire format is introduced by these Rust types. Optiflow must
separately define its durable evidence profile before adopting the adapter.

## Patch manifest

Patch SHA-256: `bff610bce37ee3ade01c33882b6f4fb92117c6390915cf18fabab3d14edb8788`.
This identifies the authored artifact; it is not test or patch-application proof.

| Upstream path | Authored change |
| --- | --- |
| `src/fd_macos.rs` | Descriptor API, bounded native bridge and decoder/encoder; seven unit cases. |
| `src/lib.rs` | 64-bit macOS-only module export. |
| `tests/test_fd_macos.rs` | Five native cases covering ordered ACL round trips, absent/empty distinction, held-descriptor identity after path replacement, invalid input/bounds, and non-filesystem descriptor refusal. |

All twelve tests are unrun. The native failure matrix still needs permission
and unsupported-volume cases, syscall/read-back failure injection, private
header flags and inheritance across final rename on both Mac architectures.

## Adoption sequence

1. Review the patch's native ABI, pointer/allocation lifetimes, absent-versus-empty
   semantics, bounds, ordered-entry representation and failure behavior.
2. Execute the authored pure and native tests on a disposable macOS/APFS fixture,
   plus upstream regressions and Rust 1.85 compilation. Tests and all other
   executable checks are deferred in this authoring checkpoint.
3. Select an accepted upstream release or explicitly reviewed, pinned dependency
   fork. Upstream publication or fork maintenance is a separate action; a patch
   in this directory is not upstream acceptance or dependency adoption.
4. Add the accepted dependency to Optiflow and define the explicit stronger
   cross-volume property-evidence profile. Keep existing Linux and same-volume
   Mac digest encodings stable; missing ACL evidence never means an empty ACL.
5. Wire bounded copy/restore and independent metadata read-back, then perform
   the interruption, native and disposable-volume qualification in the
   [parent contract](../MACOS_CROSS_VOLUME.md).

An ACL setter may change the destination before later read-back fails. This
adapter cannot promise rollback, file-content durability, path containment or
transaction atomicity. The caller owns the source/destination authority and
must stop before source removal on any failure.

Darwin's `DEFER_INHERIT` ACL-header flag can make a later rename change the ACL.
The first Optiflow integration must reject this flag or independently reobserve
properties after the final rename before claiming the destination committed.
Equality immediately after the setter does not cover that later transition.

## Deferred review commands

These are a future validation plan, **not passed evidence**. Use a disposable
native macOS/APFS workspace. `OPTIFLOW_CHECKOUT` must name the checkout that
contains this packet; the upstream checkout is separate.

```bash
git clone --branch "v0.13.0" --single-branch "https://github.com/byllyfish/exacl.git" "exacl-acl-review"
cd "exacl-acl-review"
git checkout --detach "fa36df1745a41e50ab0730546c04d86a8503e46b"
git apply --check "$OPTIFLOW_CHECKOUT/docs/work/111/acl-adapter/exacl-fd.patch"
git apply "$OPTIFLOW_CHECKOUT/docs/work/111/acl-adapter/exacl-fd.patch"
cargo +1.85.0 test --lib fd_macos
cargo +1.85.0 test --test test_fd_macos
cargo +1.85.0 test --all-targets
```

Record the resolved dependency lockfile and exact host/filesystem evidence;
the pinned upstream repository does not supply a lockfile. Repeat qualification
on both supported Mac architectures. No fixture should convert unavailable
required native behavior into a successful qualification result.

## Provenance and evidence

The upstream code is MIT-licensed; its notice is preserved in
[`LICENSE.exacl.txt`](LICENSE.exacl.txt). New proposal code is supplied under
that same license. Source review is the only evidence produced here. Native
compilation, tests, memory-safety tooling, patch application and all hosted CI
remain unexecuted. See the [#111 handoff](../HANDOFF.md) for the draft's broader
limits and qualification work.

Inspected primary source pins:

- [Apple Libc LP64 native declarations](https://github.com/apple-oss-distributions/Libc/blob/71bbe350ab79eef58113991d817ccc6165061a64/include/unistd.h).
- [XNU ACL structures, bounds, private flags and deferred inheritance](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/sys/kauth.h).
- [XNU attribute-list layout and volume capabilities](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/sys/attr.h).
- [XNU attribute packing and setter behavior](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/vfs/vfs_attrlist.c).
- [XNU support, absence and lower-layer normalization](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/vfs/kpi_vfs.c).
