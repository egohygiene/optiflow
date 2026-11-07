---
title: Source-preserving video candidate development checkpoint
description: Selected Linux resource guard, independent closed-container verification and private video artifact protocol; native qualification unrun.
---

# Produce one source-preserving video candidate

For a separately bounded serial folder batch using this same producer, see
[folder candidates](video-folder-candidates.md). That source checkpoint adds
read-only discovery, per-entry outcomes and conservative journal inspection;
it grants no replacement/disposal or native qualification.

The #146 development library now has `video_production::produce`, a
[versioned request](../schemas/video-production-request-v1.schema.json), an
explicit [trial example](../examples/video_candidate.rs), and a separate
`optiflow.video-candidate-artifact-set.v1` reader. This is uncompiled/unrun
source, absent from the published v0.1.1 capability. Nothing in this document
installs providers or authorizes execution. Native checks remain deferred.

## Supported source and operation

The initial production path selects stream copy only: one progressive FFV1
Matroska video track, yuv420p, even dimensions up to 640x360, square pixels,
explicit limited-range BT.709, exactly 25 fps/40ms intervals, zero origin,
at most 250 frames, 16 MiB input/candidate and 64 MiB decoded bytes per subject.
No audio, chapters, attachments, descriptive tags, unknown elements, HDR,
rotation, lacing, invisible/discardable blocks or Matroska CRC elements are
accepted. Known-size clusters and the closed EBML element allowlist are required.
Unknown source features refuse; this is not complete support for every Matroska
feature. The older lossy review planner remains inert. This production path
refuses lossy requests before provider access and chooses no quality target.

The host reads the complete source through existing no-follow file bindings,
checks the independent EBML/container/track/payload/clock view, then performs a
separately bounded full FFprobe observation and full FFmpeg YUV decode. Actual
candidate bytes undergo those checks independently. Ordered elementary packet
payloads, exact PTS/keyframe state, codec private data, track/aspect/color
contract and every decoded YUV420 sample must agree. Known encoder/duration
bookkeeping may regenerate; technical container IDs/indexes/padding can differ.
No unknown/descriptive metadata is silently dropped. No resizing, retiming,
audio transform or approximate equivalence is admitted.

Only a completely verified, strictly smaller candidate can be published while
the source/provider bindings still match. A non-shrinking result returns
`unchanged` with observed sizes and leaves the source intact. Encoded-byte
reduction is not physical space reclaimed: no original is replaced/disposed.

## Explicit Linux launch policy

Production requires Linux cgroup v2, a delegated local user systemd manager with
the v256 interface, working user namespaces/seccomp and explicitly pinned
systemd-run, systemctl and the optiflow CLI built with this guard. It does not
use a privileged system manager, scope launcher, PATH lookup, installer or
fallback. Select the actual `/run/user/UID/bus` socket and trusted complete
BLAKE3 executable digests in [the request example](../examples/video-production-request-v1.json).
Example hashes are placeholders, not observed identities.

Each version/probe/encode/decode invocation enters a new transient service with
MemoryMax, zero swap, TasksMax=8, cgroup lifetime/cleanup, bounded private tmpfs,
read-only other filesystems, no new privileges, protected cgroups and denied
network/mount syscalls. An internal safe Rust exec guard reads actual cgroup
limits, mountinfo and tmpfs capacity before executing any media tool with an
empty environment. It refuses unavailable/degraded boundaries. The only accepted
stderr is that bounded guard receipt; native warnings/errors cannot be hidden.
The guard also requires no-new-privileges, an active seccomp filter and empty
effective/bounding capabilities. This is not a general security/escaped-process
attestation; `process_tree_contained` remains false pending qualification.
Every service is stopped and independently confirmed inactive, including on
cancel/overflow/failure. A cooperative operation deadline and service runtime
limit apply; cleanup has a separate at-most-two-second allowance per invocation.
Unsupported namespace/delegation/controller/mount policies refuse rather than
weakening the selected profile. macOS production is not implemented here.

The memory scope is precise: the kernel limit covers the provider subtree;
source/candidate/decode payload buffers are bounded and admitted separately.
`limits.memory_bytes` admits those payload ceilings plus the selected provider
quota. It is **not** a kernel cap on total Rust host RSS/allocator/parser overhead.
Scratch plus worst-case publication bytes are admitted against temporary_bytes.
Free-space reserve is observed before and at commit; concurrent users can change
space and it is not reserved. Host filesystem calls/parsing are cooperative, not
preemptible or hard wall-clock guarantees. Full host memory accounting and
supported-host qualification remain residual #146/#65 work. No resource claim
is inferred merely from a requested number or successful encoder exit.

## Private durable artifact protocol

A candidate is `candidate.mkv` plus private `evidence.json` and
`video-candidate-artifact-set.json`, beneath a canonical UUID directory in an
existing private root. Descriptor/no-follow, exclusive creation, mode 0700/0600,
fsync and no-replace rename reuse the existing publication primitives. Complete
staging is never committed until rename and parent synchronization. A final
source/provider/deadline/free-space check runs just before publication.

The evidence binds the actual typed request, complete source identity/snapshot,
provider/build/argv identities, kernel receipts, independent container/packet
views and complete decoded-byte identities. `video_artifact::inspect` verifies
current marker/media/evidence joins, re-parses candidate structure and rejects
mutated, partial, symlinked or extra members. It never promotes or deletes
abandoned `.video-staging-*` state. Uncertain cleanup/durability stays a failure;
retain the named source/work/staging namespaces for explicit inspection.

This protocol grants no replacement or disposal authority and has no arbitrary
shell hook or sibling source import. Released Flow adapters own optional
before/after stage placement (#70). Re-observe accepted source/candidate state
at any future apply boundary; do not use a past evidence packet as a file lock.

## Deferred fresh-checkout synthetic trial

Authoring these commands does not run them. At #65/#146 qualification, build
with the existing locked dependencies; do not dispatch CI or fetch/install tools
as part of this implementation sprint. Generate the separate
[synthetic source](video-synthetic-input.md) using `-g:v 1 -write_crc32 0` for
this closed subset. Record actual paths/digests/versions. Create separate empty
private work/store directories with mode 0700, and fill in the versioned example
request with your own local UID, helper build and selected budgets.

```bash
# Deferred: requires explicit end of the current testing/native execution deferral.
cargo build --locked --bin optiflow --example video_candidate
cargo run --locked --example video_candidate -- /absolute/video-production-request.json
```

Inspect the returned status and private set through `video_artifact::inspect`.
Compare/play source and candidate separately; a valid remux can be larger and
there is no promised percentage reduction. Do not use private media for fixtures.
Ten synthetic/policy/protocol regressions are authored, uncompiled/unrun. All
tests, builds/compiler, lint/format, schemas/corpus/drift/docs checks,
dependency acquisition and native/provider/media/CI execution remain deferred.
No measured savings, release, actual folder processing, replacement or disposal is claimed.

Primary source references inspected while authoring: pinned
[systemd-run v256 interface](https://github.com/systemd/systemd/blob/v256/man/systemd-run.xml)
and [launcher implementation](https://github.com/systemd/systemd/blob/v256/src/run/run.c),
existing FFmpeg 8.0.1 adapter/source observation contracts and EBML/Matroska's
closed supported element subset. Actual supported-host execution remains unqualified.
